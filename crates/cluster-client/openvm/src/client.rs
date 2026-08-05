//! Remote Axiom Edge cluster proving.

use core::time::Duration;

use ere_compiler_core::Elf;
use ere_prover_core::{Input, RemoteProverConfig};
use ere_verifier_openvm::{
    OpenVMProgramVk, OpenVMProof, OpenVMVerifier, codec::Decode, zkVMVerifier,
};
use futures_util::StreamExt;
use reqwest::{Client, StatusCode, multipart};
use reqwest_eventsource::{Error as EventSourceError, Event, EventSource};
use sha2::{Digest, Sha256};
use tokio::time::{Instant, sleep, timeout_at};
use tracing::warn;
use uuid::Uuid;

use crate::{
    api::{
        CancelProofRequest, ProgramRef, ProofStateResponse, ProofStatus, StartProofRequest, StdIn,
    },
    error::Error,
};

/// Silence allowed on the event stream before the connection counts as dead.
/// Bounds each read rather than the whole request, since a proof could run for
/// minutes.
const EVENT_STREAM_READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Backoff before re-submitting a proof the cluster was too busy to accept.
const BUSY_RETRY_INTERVAL: Duration = Duration::from_secs(5);

/// Wrapper for the Axiom Edge cluster client.
#[derive(Debug)]
pub struct OpenVMClusterClient {
    elf: Elf,
    http: Client,
    /// Bounds each read instead of the whole request, which `http` caps too
    /// tightly for an event stream.
    events: Client,
    endpoint: String,
    program: ProgramRef,
    verifier: OpenVMVerifier,
}

impl OpenVMClusterClient {
    /// Connect to the manager and fetch the verifying key for the `elf`.
    ///
    /// The deployment assigns its own program, so the key is read back from the
    /// cluster rather than derived here.
    pub async fn new(config: &RemoteProverConfig, elf: Elf) -> Result<Self, Error> {
        let http = Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        let events = Client::builder()
            .read_timeout(EVENT_STREAM_READ_TIMEOUT)
            .connect_timeout(Duration::from_secs(10))
            .build()?;
        let endpoint = config.endpoint.trim_end_matches('/').to_string();
        let program = program_ref(&elf);
        let program_vk = fetch_program_vk(&http, &endpoint, &program.name).await?;
        Ok(Self {
            elf,
            http,
            events,
            endpoint,
            program,
            verifier: OpenVMVerifier::new(program_vk),
        })
    }

    /// Returns a reference to the ELF.
    pub fn elf(&self) -> &Elf {
        &self.elf
    }

    /// Returns a reference to the verifier.
    pub fn verifier(&self) -> &OpenVMVerifier {
        &self.verifier
    }

    /// Returns the program vk.
    pub fn program_vk(&self) -> &OpenVMProgramVk {
        self.verifier.program_vk()
    }

    /// Stages the input and starts a proof, returning its uuid immediately,
    /// without waiting for completion.
    ///
    /// Everything goes through the manager, since a deployment normally keeps
    /// its workers unroutable from outside.
    pub async fn create_prove_job(&self, input: &Input) -> Result<String, Error> {
        if input.proofs.is_some() {
            return Err(Error::UnsupportedInput("no dedicated proofs stream"));
        }

        // The uuid becomes a directory name on every worker, which allows only
        // alphanumerics plus `_` and `-`.
        let proof_uuid = Uuid::new_v4().simple().to_string();

        // `legacy()` is bincode 1 defaults (fixed-width ints, little endian),
        // which is what the workers decode the staged input with.
        let path = format!("/upload_input/{proof_uuid}");
        let stdin = bincode::serde::encode_to_vec(
            StdIn::from_bytes(input.stdin()),
            bincode::config::legacy(),
        )?;
        let form = multipart::Form::new().part(
            "input",
            multipart::Part::bytes(stdin).file_name("input.bin"),
        );
        let resp = self
            .http
            .post(format!("{}{path}", self.endpoint))
            .multipart(form)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Error::Status {
                path,
                status: resp.status(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        let path = "/start_proof";
        let resp = self
            .http
            .post(format!("{}{path}", self.endpoint))
            .json(&StartProofRequest {
                proof_uuid: proof_uuid.clone(),
                program: self.program.clone(),
                input_already_uploaded: false,
                timeout_secs: None,
            })
            .send()
            .await?;
        match resp.status() {
            StatusCode::OK => Ok(proof_uuid),
            // The manager runs one proof at a time, so a busy cluster is a
            // transient rather than an error.
            StatusCode::CONFLICT => {
                let body = resp.text().await.unwrap_or_default();
                if body.contains("program_not_in_loadout") {
                    Err(Error::ProgramNotRegistered {
                        program: self.program.to_string(),
                    })
                } else {
                    Err(Error::ClusterBusy)
                }
            }
            StatusCode::SERVICE_UNAVAILABLE => {
                Err(Error::NotReady(resp.text().await.unwrap_or_default()))
            }
            // A worker with no free app prover reports a 500, which is
            // transient since one draining a cancelled proof frees up.
            StatusCode::INTERNAL_SERVER_ERROR => {
                let body = resp.text().await.unwrap_or_default();
                if body.contains("failed to accept work") {
                    Err(Error::NotReady(body))
                } else {
                    Err(Error::Status {
                        path: path.to_string(),
                        status: StatusCode::INTERNAL_SERVER_ERROR,
                        body,
                    })
                }
            }
            status => Err(Error::Status {
                path: path.to_string(),
                status,
                body: resp.text().await.unwrap_or_default(),
            }),
        }
    }

    /// Waits for a proof to settle and returns it along with the cluster's
    /// self-reported proving time.
    ///
    /// The time spans job admission to completion, matching the boundary
    /// `ere-cluster-client-zisk` reports so the two stay comparable.
    pub async fn wait_prove_job(&self, proof_uuid: &str) -> Result<(OpenVMProof, Duration), Error> {
        match self.await_settled(proof_uuid).await? {
            ProofStatus::Completed => {}
            ProofStatus::Failed(reason) => {
                return Err(Error::JobFailed {
                    proof_uuid: proof_uuid.to_string(),
                    reason,
                });
            }
            ProofStatus::Canceled => {
                return Err(Error::JobCancelled {
                    proof_uuid: proof_uuid.to_string(),
                });
            }
            status => unreachable!("{status:?} is not a settled status"),
        }

        let state = self.proof_state(proof_uuid).await?;
        let proving_time = state
            .e2e_latency_ms
            .map(Duration::from_millis)
            .ok_or(Error::MissingField("e2e_latency_ms"))?;

        Ok((self.fetch_final_proof(proof_uuid).await?, proving_time))
    }

    /// Reads the cluster's event stream until the proof settles.
    ///
    /// The stream replays the current status on subscribe, so an
    /// [`EventSource`] reconnect cannot miss a transition.
    async fn await_settled(&self, proof_uuid: &str) -> Result<ProofStatus, Error> {
        let path = format!("/proof_events/{proof_uuid}");
        let request = self.events.get(format!("{}{path}", self.endpoint));
        let mut events =
            EventSource::new(request).map_err(|e| Error::EventStream(e.to_string()))?;

        while let Some(event) = events.next().await {
            match event {
                Ok(Event::Open) => {}
                Ok(Event::Message(message)) => {
                    let status: ProofStatus = serde_json::from_str(&message.data)
                        .map_err(|e| Error::DecodeEvent(message.data.clone(), e))?;
                    if status.is_settled() {
                        events.close();
                        return Ok(status);
                    }
                }
                // A status the cluster will never serve, so retrying is futile.
                Err(EventSourceError::InvalidStatusCode(status, resp)) => {
                    events.close();
                    return Err(Error::Status {
                        path,
                        status,
                        body: resp.text().await.unwrap_or_default(),
                    });
                }
                // Anything else is a broken connection, which `EventSource`
                // reopens on its own.
                Err(e) => warn!(proof_uuid, "event stream interrupted: {e}, reconnecting..."),
            }
        }

        Err(Error::EventStream(format!(
            "the event stream for {proof_uuid} ended before the proof settled"
        )))
    }

    /// Cancels a proof.
    ///
    /// Returns `false` if the proof is already in a terminal state.
    pub async fn cancel_prove_job(&self, proof_uuid: &str) -> Result<bool, Error> {
        let resp = self
            .http
            .post(format!("{}/cancel_proof", self.endpoint))
            .json(&CancelProofRequest {
                proof_uuid: proof_uuid.to_string(),
            })
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    /// Submits a proof, waits for completion, cancels the proof on deadline.
    ///
    /// Retries submission while the cluster is busy or its workers are not yet
    /// ready, which is what absorbs their AOT compile.
    ///
    /// Returns `Error::CreateProveJobTimeout` if the deadline expires before
    /// the submission, or `Error::ProveTimeout` if it expires before the proof.
    pub async fn prove(
        &self,
        input: &Input,
        deadline: Instant,
    ) -> Result<(OpenVMProof, Duration), Error> {
        let submit = async {
            loop {
                match self.create_prove_job(input).await {
                    Ok(proof_uuid) => return Ok(proof_uuid),
                    Err(Error::ClusterBusy) => sleep(BUSY_RETRY_INTERVAL).await,
                    // A program the deployment does not serve is a configuration
                    // mismatch, which retrying cannot repair.
                    Err(err @ Error::ProgramNotRegistered { .. }) => return Err(err),
                    Err(Error::NotReady(message)) => {
                        warn!(message, "cluster not ready, retrying...");
                        sleep(BUSY_RETRY_INTERVAL).await;
                    }
                    Err(err) => return Err(err),
                }
            }
        };

        let proof_uuid = match timeout_at(deadline, submit).await {
            Ok(result) => result?,
            Err(_) => return Err(Error::CreateProveJobTimeout),
        };

        match timeout_at(deadline, self.wait_prove_job(&proof_uuid)).await {
            Ok(result) => result,
            Err(_) => {
                let _ = self.cancel_prove_job(&proof_uuid).await;
                Err(Error::ProveTimeout { proof_uuid })
            }
        }
    }

    async fn proof_state(&self, proof_uuid: &str) -> Result<ProofStateResponse, Error> {
        let path = format!("/proof_state/{proof_uuid}");
        let resp = self
            .http
            .get(format!("{}{path}", self.endpoint))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Error::Status {
                path,
                status: resp.status(),
                body: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(resp.json().await?)
    }

    async fn fetch_final_proof(&self, proof_uuid: &str) -> Result<OpenVMProof, Error> {
        let path = format!("/proof/{proof_uuid}");
        let resp = self
            .http
            .get(format!("{}{path}", self.endpoint))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(Error::Status {
                path,
                status: resp.status(),
                body: resp.text().await.unwrap_or_default(),
            });
        }
        Ok(OpenVMProof::decode_from_slice(&resp.bytes().await?)?)
    }
}

/// Program version every deployment serves.
///
/// The name already identifies the program, and the cluster only uses the
/// version as a path component and a metrics label.
const PROGRAM_VERSION: u32 = 0;

/// Derives the cluster-side program identity from the ELF.
///
/// The deployment derives the same name from the ELF it staged, so a name it
/// does not know means the two are on different guests.
fn program_ref(elf: &Elf) -> ProgramRef {
    let elf_digest = Sha256::digest(&elf.0);
    ProgramRef {
        name: format!(
            "program-{:016x}",
            u64::from_be_bytes(elf_digest[..8].try_into().expect("8 bytes"))
        ),
        version: PROGRAM_VERSION,
    }
}

/// Reads the program's verifying key back from the cluster.
///
/// A guest the deployment does not serve has no key, which it reports as a
/// `404`.
async fn fetch_program_vk(
    http: &Client,
    endpoint: &str,
    name: &str,
) -> Result<OpenVMProgramVk, Error> {
    let path = format!("/vk/{name}");
    let resp = http.get(format!("{endpoint}{path}")).send().await?;
    if resp.status() == StatusCode::NOT_FOUND {
        return Err(Error::ProgramNotRegistered {
            program: name.to_string(),
        });
    }
    if !resp.status().is_success() {
        return Err(Error::Status {
            path,
            status: resp.status(),
            body: resp.text().await.unwrap_or_default(),
        });
    }
    OpenVMProgramVk::decode_from_slice(&resp.bytes().await?)
        .map_err(|e| Error::DecodeProgramVk(e.to_string()))
}
