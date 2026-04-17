use crate::api::{
    self, ExecuteOk, ExecuteRequest, ExecuteResponse, ProveOk, ProveRequest, ProveResponse,
    VerifyOk, VerifyRequest, VerifyResponse, ZkvmService,
    execute_response::Result as ExecuteResult, prove_response::Result as ProveResult,
    verify_response::Result as VerifyResult,
};
use anyhow::Context;
use ere_verifier_core::codec::{Decode, Encode};
use ere_zkvm_interface::zkvm::{
    self, Input, ProgramExecutionReport, ProgramProvingReport, PublicValues, zkVM,
};
use std::sync::Arc;
use twirp::{Request, Response, TwirpErrorResponse, async_trait::async_trait, internal};

pub use api::router;

/// zkVM server that handles the request by forwarding to the underlying
/// [`zkVM`] implementation methods.
#[allow(non_camel_case_types)]
pub struct zkVMServer<T> {
    zkvm: Arc<T>,
}

impl<T: 'static + zkVM + Send + Sync> zkVMServer<T> {
    pub fn new(zkvm: T) -> Self {
        Self {
            zkvm: Arc::new(zkvm),
        }
    }

    async fn execute(
        &self,
        input: Input,
    ) -> anyhow::Result<(PublicValues, ProgramExecutionReport)> {
        let zkvm = Arc::clone(&self.zkvm);
        tokio::task::spawn_blocking(move || zkvm.execute(&input).map_err(anyhow::Error::from))
            .await
            .context("execute panicked")?
    }

    async fn prove(
        &self,
        input: Input,
    ) -> anyhow::Result<(PublicValues, zkvm::Proof<T>, ProgramProvingReport)> {
        let zkvm = Arc::clone(&self.zkvm);
        tokio::task::spawn_blocking(move || zkvm.prove(&input).map_err(anyhow::Error::from))
            .await
            .context("prove panicked")?
    }

    async fn verify(&self, proof: zkvm::Proof<T>) -> anyhow::Result<PublicValues> {
        let zkvm = Arc::clone(&self.zkvm);
        tokio::task::spawn_blocking(move || zkvm.verify(&proof).map_err(anyhow::Error::from))
            .await
            .context("verify panicked")?
    }
}

#[async_trait]
impl<T: 'static + zkVM + Send + Sync> ZkvmService for zkVMServer<T> {
    async fn execute(
        &self,
        request: Request<ExecuteRequest>,
    ) -> twirp::Result<Response<ExecuteResponse>> {
        let ExecuteRequest {
            input_stdin: stdin,
            input_proofs: proofs,
        } = request.into_body();

        let input = Input { stdin, proofs };

        let result = match self.execute(input).await {
            Ok((public_values, report)) => ExecuteResult::Ok(ExecuteOk {
                public_values: public_values.into(),
                report: bincode::serde::encode_to_vec(&report, bincode::config::legacy())
                    .map_err(serialize_report_err)?,
            }),
            Err(err) => ExecuteResult::Err(err.to_string()),
        };

        Ok(Response::new(ExecuteResponse {
            result: Some(result),
        }))
    }

    async fn prove(
        &self,
        request: Request<ProveRequest>,
    ) -> twirp::Result<Response<ProveResponse>> {
        let ProveRequest {
            input_stdin: stdin,
            input_proofs: proofs,
        } = request.into_body();

        let input = Input { stdin, proofs };

        let result = match self.prove(input).await {
            Ok((public_values, proof, report)) => ProveResult::Ok(ProveOk {
                public_values: public_values.into(),
                proof: proof
                    .encode_to_vec()
                    .map_err(|err| internal(format!("failed to encode proof: {err:?}")))?,
                report: bincode::serde::encode_to_vec(&report, bincode::config::legacy())
                    .map_err(serialize_report_err)?,
            }),
            Err(err) => ProveResult::Err(err.to_string()),
        };

        Ok(Response::new(ProveResponse {
            result: Some(result),
        }))
    }

    async fn verify(
        &self,
        request: Request<VerifyRequest>,
    ) -> twirp::Result<Response<VerifyResponse>> {
        let request = request.into_body();

        let proof = zkvm::Proof::<T>::decode_from_slice(&request.proof)
            .map_err(|err| internal(format!("failed to decode proof: {err:?}")))?;

        let result = match self.verify(proof).await {
            Ok(public_values) => VerifyResult::Ok(VerifyOk {
                public_values: public_values.into(),
            }),
            Err(err) => VerifyResult::Err(err.to_string()),
        };

        Ok(Response::new(VerifyResponse {
            result: Some(result),
        }))
    }
}

fn serialize_report_err(err: bincode::error::EncodeError) -> TwirpErrorResponse {
    internal(format!("failed to serialize report: {err}"))
}
