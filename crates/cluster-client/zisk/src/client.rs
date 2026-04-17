//! Remote ZisK cluster proving.

use crate::error::Error;
use ere_prover_core::{Input, RemoteProverConfig, block_on};
use futures_util::StreamExt;
use std::time::Duration;
use tonic::transport::Channel;
use tracing::debug;
use zisk_distributed_grpc_api::{
    ErrorResponse, HintsMode, InputMode, LaunchProofRequest, ProofStatusType,
    SubscribeToProofRequest, SystemStatusRequest, launch_proof_response, system_status_response,
    zisk_distributed_api_client::ZiskDistributedApiClient,
};
use zisk_sdk::ZiskProofWithPublicValues;

/// Wrapper for the ZisK cluster client.
///
/// Connects to the ZisK cluster via gRPC and submits proof jobs.
pub struct ZiskClusterClient {
    client: ZiskDistributedApiClient<Channel>,
}

impl ZiskClusterClient {
    /// Create a new `ZiskClusterClient` that connects to the cluster.
    pub fn new(config: &RemoteProverConfig) -> Result<Self, Error> {
        let client = block_on(connect(&config.endpoint))?;
        Ok(Self { client })
    }

    /// Send proof request to cluster and wait for completion.
    ///
    /// Returns the proof with public values and proving time reported by the cluster.
    pub async fn prove(
        &self,
        input: &Input,
    ) -> Result<(ZiskProofWithPublicValues, Duration), Error> {
        let mut client = self.client.clone();

        // Check system status to get available compute capacity

        debug!("Checking system status...");

        let status_response = client.system_status(SystemStatusRequest {}).await?;

        let compute_capacity = match status_response.into_inner().result {
            Some(system_status_response::Result::Status(status)) => {
                debug!(
                    total_workers = status.total_workers,
                    compute_capacity = status.compute_capacity,
                    idle_workers = status.idle_workers,
                    busy_workers = status.busy_workers,
                    active_jobs = status.active_jobs,
                    "System status",
                );

                if status.total_workers == 0 || status.compute_capacity == 0 {
                    return Err(cluster_error("No worker available in the cluster"));
                }
                if status.active_jobs != 0 {
                    return Err(cluster_error("Cluster is busy with another proof job"));
                }

                status.compute_capacity
            }
            Some(system_status_response::Result::Error(res)) => {
                return Err(cluster_error_from_response("System status error", res));
            }
            None => {
                return Err(cluster_error("Received empty system status response"));
            }
        };

        // Launch proof

        let data_id = uuid::Uuid::new_v4().to_string();

        debug!(data_id = data_id, "Launching proof...");

        let launch_request = LaunchProofRequest {
            data_id,
            compute_capacity,
            minimal_compute_capacity: compute_capacity,
            inputs_mode: InputMode::Data.into(),
            inputs_uri: None,
            input_data: Some(framed_stdin(input.stdin())),
            hints_mode: HintsMode::None.into(),
            hints_uri: None,
            simulated_node: None,
        };

        let launch_response = client.launch_proof(launch_request).await?;

        let job_id = match launch_response.into_inner().result {
            Some(launch_proof_response::Result::JobId(job_id)) => {
                debug!(job_id = job_id, "Proof launched successfully");

                job_id
            }
            Some(launch_proof_response::Result::Error(res)) => {
                return Err(cluster_error_from_response("Launch proof error", res));
            }
            None => {
                return Err(cluster_error("Received empty launch proof response"));
            }
        };

        // Subscribe to proof status updates

        debug!(job_id = job_id, "Subscribing to proof status updates...");

        let stream = client
            .subscribe_to_proof(SubscribeToProofRequest { job_id })
            .await?;

        // Wait for proof status update (completion or failure)

        debug!("Waiting for proof status update (completion or failure)...");

        if let Some(update) = stream.into_inner().next().await {
            let update = update.map_err(cluster_error)?;

            match ProofStatusType::try_from(update.status) {
                Ok(ProofStatusType::ProofStatusCompleted) => match update.final_proof {
                    Some(final_proof) => {
                        let proof_with_publics = ZiskProofWithPublicValues::new_from_vadcop_proof(
                            &final_proof.values,
                            false,
                        )
                        .map_err(Error::InvalidProofFormat)?;
                        let proving_time = Duration::from_millis(update.duration_ms);

                        debug!(
                            proving_time = ?proving_time,
                            "Proof generated successfully"
                        );

                        Ok((proof_with_publics, proving_time))
                    }
                    None => Err(cluster_error("Missing final proof")),
                },
                Ok(ProofStatusType::ProofStatusFailed) => Err(update
                    .error
                    .map(|res| cluster_error_from_response("Proof generation error", res))
                    .unwrap_or_else(|| cluster_error("Unknown error"))),
                Err(err) => Err(cluster_error(err)),
            }
        } else {
            Err(cluster_error("Stream ended without completion status"))
        }
    }
}

/// Connect to the ZisK cluster at the given gRPC endpoint.
async fn connect(endpoint: &str) -> Result<ZiskDistributedApiClient<Channel>, Error> {
    let channel = Channel::from_shared(endpoint.to_string())?
        .connect()
        .await?;
    Ok(ZiskDistributedApiClient::new(channel))
}

/// Returns `Error::Cluster`.
fn cluster_error(s: impl ToString) -> Error {
    Error::Cluster(s.to_string())
}

/// Returns `Error::Cluster` formatted with error code and message.
fn cluster_error_from_response(s: impl ToString, res: ErrorResponse) -> Error {
    Error::Cluster(format!(
        "{}, code: {}, message: {}",
        s.to_string(),
        res.code,
        res.message
    ))
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
