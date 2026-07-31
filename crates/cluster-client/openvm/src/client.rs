//! Remote Axiom Edge cluster proving.

use core::time::Duration;

use ere_compiler_core::Elf;
use ere_prover_core::{Input, RemoteProverConfig};
use ere_verifier_openvm::{
    OpenVMProgramVk, OpenVMProof, OpenVMVerifier, codec::Decode, sdk_vm_config, zkVMVerifier,
};
use futures_util::StreamExt;
use openvm_sdk::{
    Sdk,
    config::{AggregationSystemParams, AppConfig},
    types::ExecutableFormat,
};
use openvm_stark_sdk::config::{MAX_APP_LOG_STACKED_HEIGHT, app_params_with_100_bits_security};
use reqwest::{Client, StatusCode, multipart};
use reqwest_eventsource::{Error as EventSourceError, Event, EventSource};
use sha2::{Digest, Sha256};
use tokio::time::{Instant, sleep, timeout_at};
use tracing::warn;

use crate::{
    api::{CancelProofRequest, ProgramRef, ProofStateResponse, ProofStatus, StartProofRequest},
    error::Error,
};

/// How long the event stream may go silent before the client treats the
/// connection as dead. The manager's keep-alive is far more frequent, so this
/// only fires on a genuinely broken connection. It is a per-read timeout
/// rather than a whole-request one, since a proof legitimately runs for hours.
const EVENT_STREAM_READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Backoff before re-submitting a proof the cluster was too busy to accept.
const BUSY_RETRY_INTERVAL: Duration = Duration::from_secs(5);

/// Client for a self-hosted Axiom Edge cluster proving OpenVM programs.
///
/// The deployment assigns its program before it accepts work, so [`Self::new`]
/// derives the verifying key from the guest ELF locally rather than uploading
/// anything. The program name is a digest of that ELF, so a deployment serving
/// a different guest rejects the proof request by name instead of returning a
/// proof the verifier would refuse.
///
/// A worker still has to ahead-of-time compile the guest before it can prove,
/// so [`Self::prove`] absorbs that wait by retrying the submission while the
/// manager reports the workers as not ready, then follows the proof over the
/// cluster's event stream rather than polling it.
#[derive(Debug)]
pub struct OpenVMClusterClient {
    elf: Elf,
    http: Client,
    /// Separate from `http` because that client caps whole requests, which a
    /// proof's event stream outlives. This one bounds each read instead.
    events: Client,
    endpoint: String,
    program: ProgramRef,
    verifier: OpenVMVerifier,
}

impl OpenVMClusterClient {
    /// Connect to the manager and derive the verifying key for `elf`.
    ///
    /// The VM config is always [`sdk_vm_config`], since the proof is verified
    /// against a key this crate's verifier holds and only that config produces
    /// a matching one.
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
        let vm_config =
            serde_json::to_string(&sdk_vm_config()).map_err(Error::SerializeVmConfig)?;
        let program = program_ref(&elf, &vm_config);

        let program_vk = derive_program_vk(&elf)?;

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

    /// Stages `input` on the manager and starts a proof, returning its uuid
    /// without waiting for completion.
    ///
    /// The input goes to the manager, which fans it out to the workers, so the
    /// client never needs to reach a worker directly. That matters because a
    /// deployment normally keeps its workers on a private network where their
    /// registered URLs are unroutable from outside.
    pub async fn create_prove_job(&self, input: &Input) -> Result<String, Error> {
        if input.proofs.is_some() {
            return Err(Error::MissingField("no dedicated proofs stream"));
        }

        // The uuid becomes a directory name on every worker, so keep it inside
        // the worker's allowlist of alphanumerics plus `_` and `-`.
        let proof_uuid = format!("ere-{}", uuid::Uuid::new_v4().simple());

        // Staged as compact bytes: the workers wrap them into a `StdIn`, which
        // keeps the OpenVM proving types out of this client entirely.
        let path = format!("/upload_input/{proof_uuid}");
        let form = multipart::Form::new().part(
            "input_compact",
            multipart::Part::bytes(input.stdin().to_vec()).file_name("input.bin"),
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
            // The manager runs one proof at a time, so a busy cluster is an
            // expected transient rather than an error.
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
            // A worker with no free app prover is reported as a 500. It is
            // transient, since one still draining a cancelled proof frees up.
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

    /// Waits for a proof to settle and returns it along with the cluster's own
    /// proving time.
    ///
    /// Subscribes to the cluster's event stream rather than polling. The
    /// timings are read once at the end, since the events carry the status
    /// alone.
    ///
    /// The reported time spans job admission to completion, so it includes the
    /// manager's input fan-out to the workers. The cluster also reports a
    /// narrower proving-only figure, which is deliberately not used because
    /// `ere-cluster-client-zisk` reports the wider boundary and the two are
    /// compared against each other.
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
                return Err(Error::JobCanceled {
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
    /// The stream replays the current status on subscribe, so the reconnects
    /// [`EventSource`] performs on a dropped connection cannot miss a
    /// transition.
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

    /// Cancels a proof. Returns `false` if it already reached a terminal state.
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

    /// Submits a proof, waits for completion, and cancels it on deadline.
    ///
    /// Retries submission every 5 seconds while the cluster is busy with
    /// another proof or reports its workers as not yet ready, and re-registers
    /// immediately when the manager has lost the program, all until the
    /// deadline. The not-ready retry is what absorbs the workers' AOT compile.
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
                    // The deployment assigns its loadout, so a program it does
                    // not serve is a configuration mismatch the client cannot
                    // repair by retrying.
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

/// Derives the cluster-side program identity from the artifacts that define it.
///
/// The name binds the ELF and the version binds the VM config, so a different
/// guest or a different config is a distinct program on the cluster rather
/// than a silent overwrite of an existing one.
///
/// A deployment assigns its program up front and derives this same name from
/// the ELF it staged, so a name the cluster does not know means the client and
/// the deployment are on different guests.
fn program_ref(elf: &Elf, vm_config: &str) -> ProgramRef {
    let elf_digest = Sha256::digest(&elf.0);
    let config_digest = Sha256::digest(vm_config.as_bytes());
    ProgramRef {
        name: format!(
            "program-{:016x}",
            u64::from_be_bytes(elf_digest[..8].try_into().expect("8 bytes"))
        ),
        version: u32::from_be_bytes(config_digest[..4].try_into().expect("4 bytes")),
    }
}

/// Derives the program's verifying key from its ELF, without asking the
/// cluster for one.
///
/// The deployment assigns its program up front and builds its keyset from the
/// same VM config and system params used here, so the baseline derived locally
/// is the one its proofs commit to. A drift on either side surfaces as a proof
/// that fails to verify rather than as a mismatch reported here.
fn derive_program_vk(elf: &Elf) -> Result<OpenVMProgramVk, Error> {
    let app_params = app_params_with_100_bits_security(MAX_APP_LOG_STACKED_HEIGHT);
    let sdk = Sdk::new(
        AppConfig::new(sdk_vm_config(), app_params),
        AggregationSystemParams::default(),
    )
    .map_err(|e| Error::DeriveProgramVk(e.to_string()))?;
    let exe = sdk
        .convert_to_exe(ExecutableFormat::from(&elf.0[..]))
        .map_err(|e| Error::DeriveProgramVk(e.to_string()))?;
    let baseline = sdk
        .prover(exe)
        .map_err(|e| Error::DeriveProgramVk(e.to_string()))?
        .generate_baseline();
    Ok(OpenVMProgramVk::new(baseline))
}
