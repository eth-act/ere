use std::time::{Duration, Instant};

use metrics::{counter, gauge, histogram};
use metrics_exporter_prometheus::{BuildError, PrometheusBuilder, PrometheusHandle};
use twirp::axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

const INFO: &str = "ere_server_info";
const HTTP_REQUESTS_TOTAL: &str = "ere_server_http_requests_total";
const HTTP_REQUEST_DURATION_SECONDS: &str = "ere_server_http_request_duration_seconds";
const HTTP_REQUESTS_IN_FLIGHT: &str = "ere_server_http_requests_in_flight";
const EXECUTE_TOTAL: &str = "ere_server_execute_total";
const EXECUTE_DURATION_SECONDS: &str = "ere_server_execute_duration_seconds";
const PROVE_TOTAL: &str = "ere_server_prove_total";
const PROVE_DURATION_SECONDS: &str = "ere_server_prove_duration_seconds";
const PROVE_PROOF_BYTES: &str = "ere_server_prove_proof_bytes";
const VERIFY_TOTAL: &str = "ere_server_verify_total";
const VERIFY_DURATION_SECONDS: &str = "ere_server_verify_duration_seconds";

pub fn init(
    zkvm_name: &'static str,
    zkvm_sdk_version: &'static str,
) -> Result<PrometheusHandle, BuildError> {
    let handle = PrometheusBuilder::new().install_recorder()?;

    gauge!(
        INFO,
        "version" => env!("CARGO_PKG_VERSION"),
        "zkvm_name" => zkvm_name,
        "zkvm_sdk_version" => zkvm_sdk_version,
    )
    .set(1.0);

    Ok(handle)
}

pub fn spawn_upkeep(handle: PrometheusHandle) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            handle.run_upkeep();
        }
    });
}

pub fn record_execute<T, E>(result: &Result<T, E>, elapsed: Duration) {
    record_call(EXECUTE_TOTAL, EXECUTE_DURATION_SECONDS, result, elapsed);
}

pub fn record_prove<T, E>(result: &Result<T, E>, elapsed: Duration) {
    record_call(PROVE_TOTAL, PROVE_DURATION_SECONDS, result, elapsed);
}

pub fn record_prove_proof_bytes(len: usize) {
    histogram!(PROVE_PROOF_BYTES).record(len as f64);
}

pub fn record_verify<T, E>(result: &Result<T, E>, elapsed: Duration) {
    record_call(VERIFY_TOTAL, VERIFY_DURATION_SECONDS, result, elapsed);
}

fn record_call<T, E>(
    total: &'static str,
    duration: &'static str,
    result: &Result<T, E>,
    elapsed: Duration,
) {
    let status = if result.is_ok() { "success" } else { "error" };
    counter!(total, "status" => status).increment(1);
    histogram!(duration).record(elapsed.as_secs_f64());
}

pub async fn middleware(request: Request, next: Next) -> Response {
    struct InFlightGuard {
        route: &'static str,
    }

    impl InFlightGuard {
        fn new(route: &'static str) -> Self {
            gauge!(HTTP_REQUESTS_IN_FLIGHT, "route" => route).increment(1.0);
            Self { route }
        }
    }

    impl Drop for InFlightGuard {
        fn drop(&mut self) {
            gauge!(HTTP_REQUESTS_IN_FLIGHT, "route" => self.route).decrement(1.0);
        }
    }

    let method = path_to_method(request.uri().path());
    let _guard = InFlightGuard::new(method);

    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed().as_secs_f64();

    let status = response.status().as_u16().to_string();
    counter!(
        HTTP_REQUESTS_TOTAL,
        "method" => method,
        "status" => status,
    )
    .increment(1);
    histogram!(
        HTTP_REQUEST_DURATION_SECONDS,
        "method" => method,
    )
    .record(elapsed);

    response
}

pub async fn handler(State(handle): State<PrometheusHandle>) -> String {
    handle.render()
}

pub fn path_to_method(path: &str) -> &'static str {
    match path {
        "/twirp/api.ZkvmService/Execute" => "execute",
        "/twirp/api.ZkvmService/Prove" => "prove",
        "/twirp/api.ZkvmService/Verify" => "verify",
        _ => "unknown",
    }
}
