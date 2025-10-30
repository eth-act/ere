use crate::api::{
    self, ExecuteOk, ExecuteRequest, ExecuteResponse, ProveOk, ProveRequest, ProveResponse,
    VerifyOk, VerifyRequest, VerifyResponse, ZkvmService,
    execute_response::Result as ExecuteResult, prove_response::Result as ProveResult,
    verify_response::Result as VerifyResult,
};
use ere_zkvm_interface::zkvm::{Proof, ProofKind, zkVM};
use twirp::{
    Request, Response, TwirpErrorResponse, async_trait::async_trait, internal, invalid_argument,
};

pub use api::router;

/// zkVM server that handles the request by forwarding to the underlying
/// [`zkVM`] implementation methods.
#[allow(non_camel_case_types)]
pub struct zkVMServer<T> {
    zkvm: T,
}

impl<T: 'static + zkVM + Send + Sync> zkVMServer<T> {
    pub fn new(zkvm: T) -> Self {
        Self { zkvm }
    }
}

#[async_trait]
impl<T: 'static + zkVM + Send + Sync> ZkvmService for zkVMServer<T> {
    async fn execute(
        &self,
        request: Request<ExecuteRequest>,
    ) -> twirp::Result<Response<ExecuteResponse>> {
        let request = request.into_body();

        let input = request.input;

        let result = match self.zkvm.execute(&input) {
            Ok((public_values, report)) => ExecuteResult::Ok(ExecuteOk {
                public_values,
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
        let request = request.into_body();

        let input = request.input;
        let proof_kind = ProofKind::from_repr(request.proof_kind as usize)
            .ok_or_else(|| invalid_proof_kind_err(request.proof_kind))?;

        let result = match self.zkvm.prove(&input, proof_kind) {
            Ok((public_values, proof, report)) => ProveResult::Ok(ProveOk {
                public_values,
                proof: proof.as_bytes().to_vec(),
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

        let proof_kind = ProofKind::from_repr(request.proof_kind as usize)
            .ok_or_else(|| invalid_proof_kind_err(request.proof_kind))?;

        let result = match self.zkvm.verify(&Proof::new(proof_kind, request.proof)) {
            Ok(public_values) => VerifyResult::Ok(VerifyOk { public_values }),
            Err(err) => VerifyResult::Err(err.to_string()),
        };

        Ok(Response::new(VerifyResponse {
            result: Some(result),
        }))
    }
}

fn invalid_proof_kind_err(proof_kind: i32) -> TwirpErrorResponse {
    invalid_argument(format!("invalid proof kind: {proof_kind}"))
}

fn serialize_report_err(err: bincode::error::EncodeError) -> TwirpErrorResponse {
    internal(format!("failed to serialize report: {err}"))
}
