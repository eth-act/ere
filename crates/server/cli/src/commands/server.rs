use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Error};
use ere_compiler_core::Elf;
use ere_prover_core::{
    Input, ProgramExecutionReport, ProgramProvingReport, Proof, ProverResource, PublicValues,
    codec::{Decode, Encode},
    zkVMProver,
};
use ere_server_api::{
    ExecuteOk, ExecuteRequest, ExecuteResponse, ProveOk, ProveRequest, ProveResponse, VerifyOk,
    VerifyRequest, VerifyResponse, ZkvmService, execute_response::Result as ExecuteResult,
    prove_response::Result as ProveResult, router, verify_response::Result as VerifyResult,
};
use parking_lot::Mutex;
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
    sync::Semaphore,
};
use tower::ServiceBuilder;
use tower_http::{catch_panic::CatchPanicLayer, trace::TraceLayer};
use tracing::{error, info};
use twirp::{
    Request, Response, Router, TwirpErrorResponse,
    async_trait::async_trait,
    axum::{self, extract::State, middleware, routing::get},
    internal, invalid_argument,
    reqwest::StatusCode,
    server::not_found_handler,
};

use crate::{metrics, otel};

pub async fn run(
    port: u16,
    elf: Elf,
    resource: ProverResource,
    prove_timeout: Option<Duration>,
    prove_hard_timeout: Option<Duration>,
) -> Result<(), Error> {
    let resource_kind = resource.kind();
    let zkvm = crate::construct_zkvm(elf, resource)?;
    info!("initialized zkVMProver with {resource_kind} prover");

    let metrics_handle = metrics::init(zkvm.name(), zkvm.sdk_version())
        .context("failed to install metrics recorder")?;
    metrics::spawn_upkeep(metrics_handle.clone());

    let prove_state = Arc::new(ProveState::new(prove_timeout));
    let server = Arc::new(zkVMServer::new(zkvm, Arc::clone(&prove_state)));

    // Spawn global watchdog for hard timeout if configured
    if let Some(hard_timeout) = prove_hard_timeout {
        let prove_state_clone = Arc::clone(&prove_state);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                interval.tick().await;

                if let Some(started) = *prove_state_clone.started_at.lock() {
                    let elapsed = started.elapsed();
                    if elapsed > hard_timeout {
                        error!(
                            "Prove exceeded hard timeout of {:?} (elapsed: {:?}), terminating server",
                            hard_timeout, elapsed
                        );
                        std::process::exit(1);
                    }
                }
            }
        });
        info!(
            "Global prove watchdog enabled with hard timeout of {:?}",
            hard_timeout
        );
    }

    let api_middleware = ServiceBuilder::new()
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(otel::trace_layer_make_span)
                .on_request(())
                .on_response(otel::trace_layer_on_response)
                .on_failure(otel::trace_layer_on_failure),
        )
        .layer(otel::RecordCancellationLayer)
        .layer(middleware::from_fn(metrics::middleware))
        .layer(CatchPanicLayer::new());

    let app = Router::new()
        .nest("/twirp", router(server))
        .fallback(not_found_handler)
        .layer(api_middleware)
        .route("/metrics", get(metrics::handler).with_state(metrics_handle))
        .route("/health", get(health_handler).with_state(prove_state));

    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);
    let tcp_listener = TcpListener::bind(addr).await?;

    info!("listening on {}", addr);

    axum::serve(tcp_listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("shutdown gracefully");

    Ok(())
}

/// Shared state for the prove endpoint. Holds when the currently-running prove started and the
/// prove timeout above which `/health` reports the server unhealthy. A `None` started timestamp
/// means no prove is in flight. `is_timeout` is always `false` when no timeout is configured.
pub struct ProveState {
    started_at: Mutex<Option<Instant>>,
    prove_timeout: Option<Duration>,
}

impl ProveState {
    pub fn new(prove_timeout: Option<Duration>) -> Self {
        Self {
            started_at: Mutex::new(None),
            prove_timeout,
        }
    }

    /// Returns `true` if a prove has been running longer than the configured timeout.
    pub fn is_timeout(&self) -> bool {
        let Some(timeout) = self.prove_timeout else {
            return false;
        };
        match *self.started_at.lock() {
            Some(started) => started.elapsed() > timeout,
            None => false,
        }
    }
}

/// Guard for an in-flight prove. Set on construction, cleared on `Drop`.
struct ProveInFlight {
    state: Arc<ProveState>,
}

impl ProveInFlight {
    fn new(state: Arc<ProveState>) -> Self {
        *state.started_at.lock() = Some(Instant::now());
        Self { state }
    }
}

impl Drop for ProveInFlight {
    fn drop(&mut self) {
        *self.state.started_at.lock() = None;
    }
}

/// zkVMProver server that handles the request by forwarding to the underlying [`zkVMProver`]
/// implementation methods.
///
/// `prove` is gated by a binary [`Semaphore`] so only one prove runs at a time. Requests queue in
/// FIFO order, dropping a request future before the permit is acquired removes that waiter from
/// the queue.
///
/// `execute` and `verify` are assumed concurrent-safe for the underlying implementation.
#[allow(non_camel_case_types)]
pub struct zkVMServer<T> {
    zkvm: Arc<T>,
    prove_sem: Arc<Semaphore>,
    prove_state: Arc<ProveState>,
}

impl<T: 'static + zkVMProver + Send + Sync> zkVMServer<T> {
    pub fn new(zkvm: T, prove_state: Arc<ProveState>) -> Self {
        Self {
            zkvm: Arc::new(zkvm),
            prove_sem: Arc::new(Semaphore::new(1)),
            prove_state,
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
        let permit = Arc::clone(&self.prove_sem)
            .acquire_owned()
            .await
            .context("prove semaphore closed unexpectedly")?;

        let zkvm = Arc::clone(&self.zkvm);
        let prove_state = Arc::clone(&self.prove_state);
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let _in_flight = ProveInFlight::new(prove_state);
            Ok(zkvm.prove(&input)?)
        })
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

        let start = Instant::now();
        let result = self.execute(input).await;
        metrics::record_execute(&result, start.elapsed());

        let result = match result {
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

        let start = Instant::now();
        let result = self.prove(input).await;
        metrics::record_prove(&result, start.elapsed());

        let result = match result {
            Ok((public_values, proof, report)) => {
                let proof = proof
                    .encode_to_vec()
                    .map_err(|err| internal(format!("failed to encode proof: {err:?}")))?;
                metrics::record_prove_proof_bytes(proof.len());
                ProveResult::Ok(ProveOk {
                    public_values: public_values.into(),
                    proof,
                    report: bincode::serde::encode_to_vec(&report, bincode::config::legacy())
                        .map_err(serialize_report_err)?,
                })
            }
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
            .map_err(|err| invalid_argument(format!("failed to decode proof: {err:?}")))?;

        let start = Instant::now();
        let result = self.verify(proof).await;
        metrics::record_verify(&result, start.elapsed());

        let result = match result {
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

async fn health_handler(State(state): State<Arc<ProveState>>) -> StatusCode {
    if state.is_timeout() {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    }
}

async fn shutdown_signal() {
    let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT should be enabled");
    let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM should be enabled");
    tokio::select! {
        _ = sigint.recv() => info!("received SIGINT"),
        _ = sigterm.recv() => info!("received SIGTERM"),
    }
}

fn serialize_report_err(err: bincode::error::EncodeError) -> TwirpErrorResponse {
    internal(format!("failed to serialize report: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_prove_state_timeout_detection() {
        // Test soft timeout detection (used by health check)
        let timeout = Duration::from_millis(100);
        let prove_state = ProveState::new(Some(timeout));

        // Initially, no prove is running
        assert!(!prove_state.is_timeout());

        // Simulate prove start
        {
            *prove_state.started_at.lock() = Some(Instant::now());
        }

        // Immediately after start, should not timeout
        assert!(!prove_state.is_timeout());

        // Wait for timeout to expire
        thread::sleep(Duration::from_millis(150));

        // Now it should timeout
        assert!(prove_state.is_timeout());

        // Clear the started_at (simulating prove completion)
        {
            *prove_state.started_at.lock() = None;
        }

        // After clearing, should not timeout
        assert!(!prove_state.is_timeout());
    }

    #[test]
    fn test_prove_state_no_timeout_configured() {
        // When no timeout is configured, is_timeout should always return false
        let prove_state = ProveState::new(None);

        assert!(!prove_state.is_timeout());

        // Even with a prove running
        {
            *prove_state.started_at.lock() = Some(Instant::now());
        }

        assert!(!prove_state.is_timeout());
    }

    #[test]
    fn test_prove_in_flight_guard() {
        // Test that ProveInFlight guard correctly sets and clears started_at
        let prove_state = Arc::new(ProveState::new(None));

        // Initially no prove running
        assert!(prove_state.started_at.lock().is_none());

        {
            // Create guard (simulates prove start)
            let _guard = ProveInFlight::new(Arc::clone(&prove_state));

            // started_at should be set
            assert!(prove_state.started_at.lock().is_some());

            // Guard is dropped here
        }

        // After guard drop, started_at should be cleared
        assert!(prove_state.started_at.lock().is_none());
    }
}
