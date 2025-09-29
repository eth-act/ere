use crate::{
    api::{
        self, ExecuteRequest, ExecuteResponse, ProveRequest, ProveResponse, VerifyRequest,
        VerifyResponse, ZkvmService,
    },
    input::SerializedInput,
};
use twirp::{Request, Response, async_trait::async_trait, invalid_argument};
use zkvm_interface::zkVM;

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

        let input = bincode::deserialize::<SerializedInput>(&request.input)
            .map_err(|_| invalid_argument("failed to deserialize input"))?
            .into();

        let (public_values, report) = self
            .zkvm
            .execute(&input)
            .map_err(|err| invalid_argument(format!("failed to execute: {err:?}")))?;

        Ok(Response::new(ExecuteResponse {
            public_values,
            report: bincode::serialize(&report).unwrap(),
        }))
    }

    async fn prove(
        &self,
        request: Request<ProveRequest>,
    ) -> twirp::Result<Response<ProveResponse>> {
        let request = request.into_body();

        let input = bincode::deserialize::<SerializedInput>(&request.input)
            .map_err(|_| invalid_argument("failed to deserialize input"))?
            .into();

        let (public_values, proof, report) = self
            .zkvm
            .prove(&input)
            .map_err(|err| invalid_argument(format!("failed to prove: {err:?}")))?;

        Ok(Response::new(ProveResponse {
            public_values,
            proof,
            report: bincode::serialize(&report).unwrap(),
        }))
    }

    async fn verify(
        &self,
        request: Request<VerifyRequest>,
    ) -> twirp::Result<Response<VerifyResponse>> {
        let request = request.into_body();

        let public_values = self
            .zkvm
            .verify(&request.proof)
            .map_err(|err| invalid_argument(format!("failed to verify: {err:?}")))?;

        Ok(Response::new(VerifyResponse { public_values }))
    }
}
