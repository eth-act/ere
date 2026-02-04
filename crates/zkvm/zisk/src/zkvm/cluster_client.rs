//! Remote ZisK cluster proving.

use crate::zkvm::Error;
use ere_zkvm_interface::zkvm::RemoteProverConfig;
use futures_util::StreamExt;
use std::sync::OnceLock;
use std::time::Duration;
use tonic::transport::Channel;
use tracing::debug;
use zisk_distributed_grpc_api::{
    ErrorResponse, InputMode, LaunchProofRequest, ProofStatusType, SubscribeToProofRequest,
    SystemStatusRequest, launch_proof_response, system_status_response,
    zisk_distributed_api_client::ZiskDistributedApiClient,
};

/// Wrapper for the ZisK cluster client.
///
/// Connects to the ZisK cluster via gRPC and submits proof jobs.
pub struct ClusterClient {
    client: ZiskDistributedApiClient<Channel>,
}

impl ClusterClient {
    /// Create a new ClusterClient that connects to the cluster.
    pub fn new(config: &RemoteProverConfig) -> Result<Self, Error> {
        let client = block_on(connect(&config.endpoint))?;
        Ok(Self { client })
    }

    /// Sync wrapper for [`Self::prove_async`].
    pub fn prove(&self, input: &[u8]) -> Result<(Vec<u8>, Duration), Error> {
        block_on(self.prove_async(input))
    }

    /// Send proof request to cluster and wait for completion.
    ///
    /// Returns the proof and proving time reported by the cluster.
    async fn prove_async(&self, input: &[u8]) -> Result<(Vec<u8>, Duration), Error> {
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

                if status.compute_capacity == 0 {
                    return Err(cluster_error("No compute capacity available in the system"));
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
            input_mode: InputMode::Data.into(),
            input_path: None,
            input_data: Some(input.to_vec()),
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
                        let proof = bytemuck::cast_slice(&final_proof.values).to_vec();
                        let proving_time = Duration::from_millis(update.duration_ms);

                        debug!(
                            proof_size = proof.len(),
                            proving_time = ?proving_time,
                            "Proof generated successfully"
                        );

                        Ok((proof, proving_time))
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

/// Run a future to completion, reusing the current tokio runtime or creating one.
fn block_on<T>(future: impl Future<Output = T>) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => {
            static FALLBACK_RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
            FALLBACK_RT
                .get_or_init(|| tokio::runtime::Runtime::new().expect("Failed to create runtime"))
                .block_on(future)
        }
    }
}

/// Returns `Error::ClusterError`.
fn cluster_error(s: impl ToString) -> Error {
    Error::ClusterError(s.to_string())
}

/// Returns `Error::ClusterError` formatted with error code and message.
fn cluster_error_from_response(s: impl ToString, res: ErrorResponse) -> Error {
    Error::ClusterError(format!(
        "{}, code: {}, message: {}",
        s.to_string(),
        res.code,
        res.message
    ))
}
