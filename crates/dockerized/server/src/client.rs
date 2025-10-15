use crate::{
    api::{
        ExecuteRequest, ExecuteResponse, ProveRequest, ProveResponse, VerifyRequest,
        VerifyResponse, ZkvmService,
    },
    input::SerializedInput,
};
use anyhow::{Context, Error, bail};
use ere_zkvm_interface::{
    ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind, PublicValues,
};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use twirp::{Client, Request, reqwest};

pub use twirp::url::Url;

/// zkVM client of the `zkVMServer`.
#[allow(non_camel_case_types)]
pub struct zkVMClient {
    client: Client,
}

impl zkVMClient {
    pub async fn new(url: Url) -> Result<Self, Error> {
        const TIMEOUT: Duration = Duration::from_secs(300); // 5mins
        const INTERVAL: Duration = Duration::from_millis(500);

        let http_client = reqwest::Client::new();

        let start = Instant::now();
        loop {
            if start.elapsed() > TIMEOUT {
                bail!("Health check timeout after 30 seconds")
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
        input: SerializedInput,
    ) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        let input = bincode::serialize(&input).with_context(|| "Failed to serialize input")?;

        let request = Request::new(ExecuteRequest { input });

        let response = self
            .client
            .execute(request)
            .await
            .with_context(|| "Execute RPC failed")?;

        let ExecuteResponse {
            public_values,
            report,
        } = response.into_body();

        let report: ProgramExecutionReport = bincode::deserialize(&report)
            .with_context(|| "Failed to deserialize execution report")?;

        Ok((public_values, report))
    }

    pub async fn prove(
        &self,
        input: SerializedInput,
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), Error> {
        let input = bincode::serialize(&input).with_context(|| "Failed to serialize input")?;

        let request = Request::new(ProveRequest {
            input,
            proof_kind: proof_kind as i32,
        });

        let response = self
            .client
            .prove(request)
            .await
            .with_context(|| "Prove RPC failed")?;

        let ProveResponse {
            public_values,
            proof,
            report,
        } = response.into_body();

        let report: ProgramProvingReport = bincode::deserialize(&report)
            .with_context(|| "Failed to deserialize proving report")?;

        Ok((public_values, Proof::new(proof_kind, proof), report))
    }

    pub async fn verify(&self, proof: &Proof) -> Result<PublicValues, Error> {
        let request = Request::new(VerifyRequest {
            proof: proof.as_bytes().to_vec(),
            proof_kind: proof.kind() as i32,
        });

        let response = self
            .client
            .verify(request)
            .await
            .with_context(|| "Verify RPC failed")?;

        let VerifyResponse { public_values } = response.into_body();

        Ok(public_values)
    }
}
