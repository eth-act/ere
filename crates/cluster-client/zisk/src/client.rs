//! Remote ZisK cluster proving.

use core::{iter, time::Duration};

use ere_compiler_core::Elf;
use ere_prover_core::{Input, RemoteProverConfig, zkVMVerifier};
use ere_verifier_zisk::{
    PROGRAM_VK_WORDS, PUBLIC_VALUES_BYTES, VadcopFinalProof, ZiskProgramVk, ZiskProof, ZiskVerifier,
};
use serde::Deserialize;
use tokio::time::Instant;
use tonic::transport::Channel;

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
    client: ZiskCoordinatorApiClient<Channel>,
    hash_id: String,
    verifier: ZiskVerifier,
}

impl ZiskClusterClient {
    /// Connect to the coordinator and run setup for the `elf`.
    pub async fn new(config: &RemoteProverConfig, elf: Elf) -> Result<Self, Error> {
        let mut client = ZiskCoordinatorApiClient::connect(config.endpoint.clone()).await?;
        let (hash_id, program_vk) = setup(&mut client, elf).await?;
        let verifier = ZiskVerifier::new(program_vk);
        Ok(Self {
            client,
            hash_id,
            verifier,
        })
    }

    /// Returns a reference to the verifier.
    pub fn verifier(&self) -> &ZiskVerifier {
        &self.verifier
    }

    /// Returns the program vk.
    pub fn program_vk(&self) -> ZiskProgramVk {
        *self.verifier.program_vk()
    }

    /// Submits a prove job and returns its `job_id` immediately, without waiting for completion.
    pub async fn create_prove_job(&self, input: &Input) -> Result<String, Error> {
        let mut client = self.client.clone();
        let job = JobKind {
            kind: Some(job_kind::Kind::Prove(ProveRequest {
                hash_id: self.hash_id.clone(),
                input: Some(InputKind {
                    kind: Some(input_kind::Kind::Inline(InputChunk {
                        data: framed_stdin(input.stdin()),
                    })),
                }),
                proof_dest: ProofKind::StarkMinimal as i32,
                proof_timeout: None,
                hints: None,
            })),
        };
        let req = JobRequestMessage {
            job_kind: Some(job),
        };
        let job_id = client.job_request(req).await?.into_inner().job_id;
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

    /// Submit a prove job, wait up to `timeout` for completion, cancel the job on timeout.
    ///
    /// Returns `Error::ProveTimeout` if the deadline expires before the job terminates.
    pub async fn prove(
        &self,
        input: &Input,
        deadline: Option<Instant>,
    ) -> Result<(ZiskProof, Duration), Error> {
        let job_id = self.create_prove_job(input).await?;

        match deadline {
            Some(deadline) => {
                match tokio::time::timeout_at(deadline, self.wait_prove_job(&job_id)).await {
                    Ok(result) => result,
                    Err(_) => {
                        let _ = self.cancel_prove_job(&job_id).await;
                        Err(Error::ProveTimeout { job_id })
                    }
                }
            }
            _ => self.wait_prove_job(&job_id).await,
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
        })),
    };
    let req = JobRequestMessage {
        job_kind: Some(job),
    };
    let job_id = client.job_request(req).await?.into_inner().job_id;

    let resp = match tokio::time::timeout(TIMEOUT, wait_job(client, &job_id)).await {
        Ok(resp) => match resp?.kind {
            Some(job_kind_response::Kind::Setup(resp)) => resp,
            _ => Err(Error::MissingField("kind::setup"))?,
        },
        Err(_) => Err(Error::SetupTimeout { job_id })?,
    };
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
    #[derive(Deserialize)]
    enum ProofBody {
        Vadcop {
            proof: Vec<u64>,
            _zisk_vk: Vec<u64>,
            minimal: bool,
        },
        Plonk,
    }

    #[derive(Deserialize)]
    struct PublicValues {
        data: Vec<u8>,
    }

    #[derive(Deserialize)]
    struct ProgramVK {
        vk: Vec<u64>,
    }

    #[derive(Deserialize)]
    struct Proof {
        body: ProofBody,
        publics: PublicValues,
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
    if proof.publics.data.len() != PUBLIC_VALUES_BYTES {
        Err(ere_verifier_zisk::Error::InvalidPublicValueLength {
            expected: PUBLIC_VALUES_BYTES,
            got: proof.publics.data.len(),
        })?;
    };

    let public_values = {
        let to_u64 = |bytes: &[u8]| u32::from_le_bytes(bytes.try_into().unwrap()) as u64;
        iter::empty()
            .chain(proof.program_vk.vk)
            .chain(proof.publics.data.chunks_exact(4).map(to_u64))
            .collect()
    };

    let ProofBody::Vadcop {
        proof,
        minimal: true,
        ..
    } = proof.body
    else {
        return Err(ere_verifier_zisk::Error::InvalidVadcopFinalProofKind)?;
    };

    Ok(ZiskProof(VadcopFinalProof::new(proof, public_values, true)))
}
