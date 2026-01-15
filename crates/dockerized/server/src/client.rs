use crate::api::{
    ExecuteRequest, ProveRequest, VerifyRequest, ZkvmService,
    execute_response::Result as ExecuteResult, prove_response::Result as ProveResult,
    verify_response::Result as VerifyResult,
};
use ere_zkvm_interface::zkvm::{
    Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProofKind, PublicValues,
};
use thiserror::Error;
use twirp::{Client, Request, reqwest};

pub use twirp::{
    TwirpErrorResponse,
    url::{ParseError, Url},
};

#[derive(Debug, Error)]
#[allow(non_camel_case_types)]
pub enum Error {
    #[error("Invalid URL: {0}")]
    ParseUrl(#[from] ParseError),
    #[error("zkVM method error: {0}")]
    zkVM(String),
    #[error("RPC error: {0}")]
    Rpc(#[from] TwirpErrorResponse),
}

/// zkVM client of the `zkVMServer`.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct zkVMClient {
    client: Client,
}

impl zkVMClient {
    pub fn new(endpoint: Url, http_client: reqwest::Client) -> Result<Self, Error> {
        Ok(Self {
            client: Client::new(endpoint.join("twirp")?, http_client, Vec::new(), None),
        })
    }

    pub fn from_endpoint(endpoint: Url) -> Result<Self, Error> {
        Self::new(endpoint, reqwest::Client::new())
    }

    pub async fn execute(
        &self,
        input: &Input,
    ) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        let request = Request::new(ExecuteRequest {
            input_stdin: input.stdin.clone(),
            input_proofs: input.proofs.clone(),
        });

        let response = self.client.execute(request).await?;

        match response.into_body().result.ok_or_else(result_none_err)? {
            ExecuteResult::Ok(result) => Ok((
                result.public_values,
                bincode::serde::decode_from_slice(&result.report, bincode::config::legacy())
                    .map_err(deserialize_report_err)?
                    .0,
            )),
            ExecuteResult::Err(err) => Err(Error::zkVM(err)),
        }
    }

    pub async fn prove(
        &self,
        input: &Input,
        proof_kind: ProofKind,
    ) -> Result<(PublicValues, Proof, ProgramProvingReport), Error> {
        let request = Request::new(ProveRequest {
            input_stdin: input.stdin.clone(),
            input_proofs: input.proofs.clone(),
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
            ProveResult::Err(err) => Err(Error::zkVM(err)),
        }
    }

    pub async fn verify(&self, proof: &Proof) -> Result<PublicValues, Error> {
        let request = Request::new(VerifyRequest {
            proof: proof.as_bytes().to_vec(),
            proof_kind: proof.kind() as i32,
        });

        let response = self.client.verify(request).await?;

        match response.into_body().result.ok_or_else(result_none_err)? {
            VerifyResult::Ok(result) => Ok(result.public_values),
            VerifyResult::Err(err) => Err(Error::zkVM(err)),
        }
    }
}

fn result_none_err() -> TwirpErrorResponse {
    twirp::internal("response result should always be Some")
}

fn deserialize_report_err(err: bincode::error::DecodeError) -> TwirpErrorResponse {
    twirp::internal(format!("failed to deserialize report: {err}"))
}
