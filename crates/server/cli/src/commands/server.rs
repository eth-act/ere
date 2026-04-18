use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use anyhow::{Context, Error};
use ere_compiler_core::Elf;
use ere_prover_core::{
    Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProverResource, PublicValues,
    codec::{Decode, Encode},
    zkVMProver,
};
use ere_server_client::api::{
    ExecuteOk, ExecuteRequest, ExecuteResponse, ProveOk, ProveRequest, ProveResponse, VerifyOk,
    VerifyRequest, VerifyResponse, ZkvmService, execute_response::Result as ExecuteResult,
    prove_response::Result as ProveResult, router, verify_response::Result as VerifyResult,
};
use tokio::{net::TcpListener, signal};
use tower_http::catch_panic::CatchPanicLayer;
use tracing::info;
use twirp::{
    Request, Response, Router, TwirpErrorResponse,
    async_trait::async_trait,
    axum::{self, routing::get},
    internal,
    reqwest::StatusCode,
    server::not_found_handler,
};

/// zkVMProver server that handles the request by forwarding to the underlying
/// [`zkVMProver`] implementation methods.
#[allow(non_camel_case_types)]
pub struct zkVMServer<T> {
    zkvm: Arc<T>,
}

impl<T: 'static + zkVMProver + Send + Sync> zkVMServer<T> {
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
        tokio::task::spawn_blocking(move || Ok(zkvm.execute(&input)?))
            .await
            .context("execute panicked")?
    }

    async fn prove(
        &self,
        input: Input,
    ) -> anyhow::Result<(PublicValues, Proof<T>, ProgramProvingReport)> {
        let zkvm = Arc::clone(&self.zkvm);
        tokio::task::spawn_blocking(move || Ok(zkvm.prove(&input)?))
            .await
            .context("prove panicked")?
    }

    async fn verify(&self, proof: Proof<T>) -> anyhow::Result<PublicValues> {
        let zkvm = Arc::clone(&self.zkvm);
        tokio::task::spawn_blocking(move || Ok(zkvm.verify(&proof)?))
            .await
            .context("verify panicked")?
    }
}

#[async_trait]
impl<T: 'static + zkVMProver + Send + Sync> ZkvmService for zkVMServer<T> {
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

        let proof = Proof::<T>::decode_from_slice(&request.proof)
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

pub async fn run(port: u16, elf: Elf, resource: ProverResource) -> Result<(), Error> {
    let resource_kind = resource.kind().to_string();
    let zkvm = crate::construct_zkvm(elf, resource)?;
    info!("initialized zkVMProver with {resource_kind} prover");

    let server = Arc::new(zkVMServer::new(zkvm));
    let app = Router::new()
        .nest("/twirp", router(server))
        .fallback(not_found_handler)
        .layer(CatchPanicLayer::new());
    let app = crate::otel::layer(app).route("/health", get(StatusCode::OK));

    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);
    let tcp_listener = TcpListener::bind(addr).await?;

    info!("listening on {}", addr);

    axum::serve(tcp_listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("shutdown gracefully");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {
            info!("received Ctrl+C, shutting down gracefully");
        },
        _ = terminate => {
            info!("received SIGTERM, shutting down gracefully");
        },
    }
}
