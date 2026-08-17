//! Remote ZisK cluster proving.

use core::time::Duration;

use ere_compiler_core::Elf;
use ere_prover_core::{Input, RemoteProverConfig, zkVMVerifier};
use ere_verifier_zisk::{
    PROGRAM_VK_WORDS, PUBLIC_VALUES_WORDS, VADCOP_FINAL_HASH_FAMILY, VadcopFinalProof,
    ZiskProgramVk, ZiskProof, ZiskVerifier,
};
use serde::Deserialize;
use tokio::time::{Instant, sleep, timeout, timeout_at};
use tonic::{Code, transport::Channel};
use tracing::warn;

use crate::{
    api::{
        CancelJobRequest, InputChunk, InputKind, JobKind, JobKindResponse, JobRequestMessage,
        ProofKind, ProveRequest, RegisterGuestProgramRequest, SetupRequest, WaitJobResultRequest,
        input_kind, job_kind, job_kind_response, job_status,
        zisk_coordinator_api_client::ZiskCoordinatorApiClient,
    },
    error::Error,
};

/// Wrapper for the ZisK cluster client.
#[derive(Debug)]
pub struct ZiskClusterClient {
    elf: Elf,
    client: ZiskCoordinatorApiClient<Channel>,
    hash_id: String,
    verifier: ZiskVerifier,
}

impl ZiskClusterClient {
    /// Connect to the coordinator and run setup for the `elf`.
    pub async fn new(config: &RemoteProverConfig, elf: Elf) -> Result<Self, Error> {
        let mut client = ZiskCoordinatorApiClient::connect(config.endpoint.clone()).await?;
        let (hash_id, program_vk) = setup(&mut client, elf.clone()).await?;
        let verifier = ZiskVerifier::new(program_vk);
        Ok(Self {
            elf,
            client,
            hash_id,
            verifier,
        })
    }

    /// Returns a reference to the ELF.
    pub fn elf(&self) -> &Elf {
        &self.elf
    }

    /// Returns a reference to the verifier.
    pub fn verifier(&self) -> &ZiskVerifier {
        &self.verifier
    }

    /// Returns the program vk.
    pub fn program_vk(&self) -> ZiskProgramVk {
        *self.verifier.program_vk()
    }

    /// Setup the ELF.
    pub async fn setup(&self) -> Result<(), Error> {
        setup(&mut self.client.clone(), self.elf.clone()).await?;
        Ok(())
    }

    /// Submits a prove job and returns its `job_id` immediately, without waiting for completion.
    pub async fn create_prove_job(&self, input: &Input) -> Result<String, Error> {
        let mut client = self.client.clone();
        let req = JobRequestMessage {
            job_kind: Some(prove_job(&self.hash_id, input)),
        };
        let job_id = match client.job_request(req).await {
            Ok(res) => res.into_inner().job_id,
            Err(status) if status.message().contains("setup not done") => Err(Error::SetupNotDone)?,
            Err(status) if matches!(status.code(), Code::Unavailable | Code::Internal) => {
                Err(Error::ClusterUnavailable(status))?
            }
            Err(status) => Err(Error::Grpc(status))?,
        };
        Ok(job_id)
    }

    /// Waits for a prove job to reach a terminal state and returns the proof along with the
    /// self-reported proving time.
    pub async fn wait_prove_job(&self, job_id: &str) -> Result<(ZiskProof, Duration), Error> {
        let mut client = self.client.clone();
        let resp = match wait_job(&mut client, job_id).await?.kind {
            Some(job_kind_response::Kind::Prove(resp)) => resp,
            _ => Err(Error::MissingField("kind::prove"))?,
        };
        let proof = parse_proof(&resp.proof.ok_or(Error::MissingField("proof"))?.data)?;
        let proving_time = Duration::from_nanos(
            resp.stats
                .ok_or(Error::MissingField("stats"))?
                .duration_nanos,
        );
        Ok((proof, proving_time))
    }

    /// Cancels a prove job.
    ///
    /// Returns `false` if the job is already in a terminal state.
    pub async fn cancel_prove_job(&self, job_id: &str) -> Result<bool, Error> {
        let mut client = self.client.clone();
        let req = CancelJobRequest {
            job_id: job_id.to_string(),
        };
        let cancelled = client.cancel_job(req).await?.into_inner().cancelled;
        Ok(cancelled)
    }

    /// Submits a prove job, wait for completion, cancel the job on deadline.
    ///
    /// Returns `Error::ProveTimeout` if the deadline expires before the job terminates.
    ///
    /// Retries prove job submission on every 5 seconds until deadline.
    ///
    /// Returns `Error::CreateProveJobTimeout` if the deadline expires before the job submission.
    pub async fn prove(
        &self,
        input: &Input,
        deadline: Instant,
    ) -> Result<(ZiskProof, Duration), Error> {
        let fut = async {
            loop {
                match self.create_prove_job(input).await {
                    Ok(job_id) => return Ok(job_id),
                    Err(Error::SetupNotDone) => self.setup().await?,
                    Err(Error::ClusterUnavailable(status)) => {
                        warn!(?status, "job submission failed, retrying...");
                        sleep(Duration::from_secs(5)).await;
                    }
                    Err(err) => return Err(err),
                };
            }
        };

        let job_id = match timeout_at(deadline, fut).await {
            Ok(result) => result?,
            Err(_) => Err(Error::CreateProveJobTimeout)?,
        };

        match timeout_at(deadline, self.wait_prove_job(&job_id)).await {
            Ok(result) => result,
            Err(_) => {
                let _ = self.cancel_prove_job(&job_id).await;
                Err(Error::ProveTimeout { job_id })
            }
        }
    }
}

async fn setup(
    client: &mut ZiskCoordinatorApiClient<Channel>,
    elf: Elf,
) -> Result<(String, ZiskProgramVk), Error> {
    /// Timeout for setup job.
    const TIMEOUT: Duration = Duration::from_secs(600);

    let hash_id = client
        .register_guest_program(RegisterGuestProgramRequest { zisk_elf: elf.0 })
        .await?
        .into_inner()
        .hash_id;

    let job = JobKind {
        kind: Some(job_kind::Kind::Setup(SetupRequest {
            hash_id: hash_id.clone(),
            with_hints: false,
            program_name: String::new(),
            emulator_only: false,
        })),
    };
    let req = JobRequestMessage {
        job_kind: Some(job),
    };
    let job_id = client.job_request(req).await?.into_inner().job_id;

    let resp = match timeout(TIMEOUT, wait_job(client, &job_id)).await {
        Ok(resp) => match resp?.kind {
            Some(job_kind_response::Kind::Setup(resp)) => resp,
            _ => Err(Error::MissingField("kind::setup"))?,
        },
        Err(_) => Err(Error::SetupTimeout { job_id })?,
    };

    if !resp.hash_mode.is_empty() && resp.hash_mode != VADCOP_FINAL_HASH_FAMILY {
        Err(Error::UnexpectedHashFamily {
            expected: VADCOP_FINAL_HASH_FAMILY,
            got: resp.hash_mode,
        })?;
    }

    let program_vk = ZiskProgramVk::try_from(resp.vk.as_slice())?;
    Ok((hash_id, program_vk))
}

async fn wait_job(
    client: &mut ZiskCoordinatorApiClient<Channel>,
    job_id: &str,
) -> Result<JobKindResponse, Error> {
    /// Server-side hold per `WaitJobResult`.
    const TIMEOUT_SECS: u32 = 5;

    let req = WaitJobResultRequest {
        job_id: job_id.to_string(),
        timeout_seconds: Some(TIMEOUT_SECS),
    };
    loop {
        let resp = client.wait_job_result(req.clone()).await?.into_inner();

        let status = resp
            .job_status
            .and_then(|s| s.status)
            .ok_or(Error::MissingField("job_status"))?;
        match status {
            job_status::Status::Completed(_) => {
                return resp.result.ok_or(Error::MissingField("result"));
            }
            job_status::Status::Failed(failed) => {
                return Err(Error::JobFailed {
                    job_id: job_id.to_string(),
                    reason: format!("{failed:?}"),
                });
            }
            job_status::Status::Cancelled(_) => {
                return Err(Error::JobCancelled(job_id.to_string()));
            }
            job_status::Status::Queued(_)
            | job_status::Status::Running(_)
            | job_status::Status::WaitingForInput(_) => continue,
        }
    }
}

fn prove_job(hash_id: &str, input: &Input) -> JobKind {
    JobKind {
        kind: Some(job_kind::Kind::Prove(ProveRequest {
            hash_id: hash_id.to_string(),
            input: Some(InputKind {
                kind: Some(input_kind::Kind::Inline(InputChunk {
                    data: framed_stdin(input.stdin()),
                })),
            }),
            proof_dest: ProofKind::StarkMinimal as i32,
            proof_timeout: None,
            hints: None,
        })),
    }
}

/// Returns `data` with a LE u64 length prefix and padding to multiple of 8.
///
/// The length prefix and padding is expected by ZisK emulator/prover runtime.
fn framed_stdin(data: &[u8]) -> Vec<u8> {
    let len = (8 + data.len()).next_multiple_of(8);
    let mut buf = Vec::with_capacity(len);
    buf.extend_from_slice(&(data.len() as u64).to_le_bytes());
    buf.extend_from_slice(data);
    buf.resize(len, 0);
    buf
}

fn parse_proof(bytes: &[u8]) -> Result<ZiskProof, Error> {
    /// Mirrors `zisk_common::VadcopKind`, whose discriminant order fixes the
    /// encoding. Only `Minimal` is accepted below.
    #[derive(Deserialize, PartialEq)]
    enum VadcopKind {
        Final,
        Recurser,
        Minimal,
    }

    #[derive(Deserialize)]
    enum ProofBody {
        Vadcop {
            proof: Vec<u64>,
            _zisk_vk: Vec<u64>,
            kind: VadcopKind,
            hash: String,
            /// Canonical flag-free `[program_vk(4) | inputs(64)]` at full u64
            /// width. A minimal proof carries no `is_vadcop_final_proof` flag,
            /// so this is already the vector the verifier commits to.
            publics_full: Vec<u64>,
        },
        Plonk,
    }

    #[derive(Deserialize)]
    struct ProgramVK {
        vk: Vec<u64>,
    }

    #[derive(Deserialize)]
    struct Proof {
        body: ProofBody,
        program_vk: ProgramVK,
    }

    let (proof, _): (Proof, _) =
        bincode::serde::decode_from_slice(bytes, bincode::config::standard())?;

    if proof.program_vk.vk.len() != PROGRAM_VK_WORDS {
        Err(ere_verifier_zisk::Error::InvalidProgramVkLength {
            expected: PROGRAM_VK_WORDS * 8,
            got: proof.program_vk.vk.len() * 8,
        })?;
    };
    let ProofBody::Vadcop {
        proof,
        kind,
        hash,
        publics_full,
        ..
    } = proof.body
    else {
        return Err(ere_verifier_zisk::Error::InvalidVadcopFinalProofKind)?;
    };
    if kind != VadcopKind::Minimal || hash != VADCOP_FINAL_HASH_FAMILY {
        Err(ere_verifier_zisk::Error::InvalidVadcopFinalProofKind)?;
    }
    if publics_full.len() != PROGRAM_VK_WORDS + PUBLIC_VALUES_WORDS {
        Err(ere_verifier_zisk::Error::InvalidPublicValueLength {
            expected: PROGRAM_VK_WORDS + PUBLIC_VALUES_WORDS,
            got: publics_full.len(),
        })?;
    };

    Ok(ZiskProof(VadcopFinalProof::new(
        proof,
        publics_full,
        true,
        VADCOP_FINAL_HASH_FAMILY.to_string(),
    )))
}
