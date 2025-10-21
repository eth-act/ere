use crate::api::{
    ExecuteRequest, ProveRequest, VerifyRequest, ZkvmService,
    execute_response::Result as ExecuteResult, prove_response::Result as ProveResult,
    verify_response::Result as VerifyResult,
};
use ere_zkvm_interface::{
    ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind, PublicValues,
};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::time::sleep;
use twirp::{Client, Request, reqwest};

pub use twirp::{TwirpErrorResponse, url::Url};

#[derive(Debug, Error)]
#[allow(non_camel_case_types)]
pub enum zkVMClientError {
    #[error("zkVM method error: {0}")]
    zkVM(String),
    #[error("Connection to zkVM server timeout after 5 minutes")]
    ConnectionTimeout,
    #[error("RPC error: {0}")]
    Rpc(#[from] TwirpErrorResponse),
}

/// zkVM client of the `zkVMServer`.
#[allow(non_camel_case_types)]
pub struct zkVMClient {
    client: Client,
}

impl zkVMClient {
    pub async fn new(url: Url) -> Result<Self, zkVMClientError> {
        const TIMEOUT: Duration = Duration::from_secs(300); // 5mins
        const INTERVAL: Duration = Duration::from_millis(500);

        let http_client = reqwest::Client::new();

        let start = Instant::now();
        loop {
            if start.elapsed() > TIMEOUT {
                return Err(zkVMClientError::ConnectionTimeout);
            }

            match http_client.get(url.join("health").unwrap()).send().await {
                Ok(response) if response.status().is_success() => break,
                _ => sleep(INTERVAL).await,
            }
        }

        let client = Client::new(url.join("twirp").unwrap(), http_client, Vec::new(), None);

        Ok(Self { client })
    }

    pub async fn execute(
        &self,
        input: Vec<u8>,
    ) -> Result<(PublicValues, ProgramExecutionReport), zkVMClientError> {
        let request = Request::new(ExecuteRequest { input });

        let response = self.client.execute(request).await?;

        match response.into_body().result.ok_or_else(result_none_err)? {
            ExecuteResult::Ok(result) => Ok((
                result.public_values,
                bincode::serde::decode_from_slice(&result.report, bincode::config::legacy())
                    .map_err(deserialize_report_err)?
                    .0,
            )),
            ExecuteResult::Err(err) => Err(zkVMClientError::zkVM(err)),
        }
    }

    pub async fn prove(
        &self,
        input: Vec<u8>,
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), zkVMClientError> {
        let request = Request::new(ProveRequest {
            input,
            proof_kind: proof_kind as i32,
        });

        let response = self.client.prove(request).await?;

        match response.into_body().result.ok_or_else(result_none_err)? {
            ProveResult::Ok(result) => Ok((
                result.public_values,
                Proof::new(proof_kind, result.proof),
                bincode::serde::decode_from_slice(&result.report, bincode::config::legacy())
                    .map_err(deserialize_report_err)?
                    .0,
            )),
            ProveResult::Err(err) => Err(zkVMClientError::zkVM(err)),
        }
    }

    pub async fn verify(&self, proof: &Proof) -> Result<PublicValues, zkVMClientError> {
        let request = Request::new(VerifyRequest {
            proof: proof.as_bytes().to_vec(),
            proof_kind: proof.kind() as i32,
        });

        let response = self.client.verify(request).await?;

        match response.into_body().result.ok_or_else(result_none_err)? {
            VerifyResult::Ok(result) => Ok(result.public_values),
            VerifyResult::Err(err) => Err(zkVMClientError::zkVM(err)),
        }
    }
}

fn result_none_err() -> TwirpErrorResponse {
    twirp::internal("response result should always be Some")
}

fn deserialize_report_err(err: bincode::error::DecodeError) -> TwirpErrorResponse {
    twirp::internal(format!("failed to deserialize report: {err}"))
}
