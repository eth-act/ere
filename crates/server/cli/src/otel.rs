use core::{
    pin::Pin,
    task::{Context, Poll, ready},
    time::Duration,
};
use std::env;

use opentelemetry::{propagation::Extractor, trace::TracerProvider};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{SdkTracer, SdkTracerProvider},
};
use pin_project_lite::pin_project;
use tower::{Layer, Service};
use tower_http::classify::ServerErrorsFailureClass;
use tracing::{Span, error, field::Empty, info, info_span, warn};
use tracing_opentelemetry::{OpenTelemetryLayer, OpenTelemetrySpanExt};
use tracing_subscriber::Registry;
use twirp::axum::{extract::Request, http::HeaderMap, response::Response};

use crate::metrics::path_to_method;

pub type OtelLayer = OpenTelemetryLayer<Registry, SdkTracer>;

pub fn init() -> (Option<SdkTracerProvider>, Option<OtelLayer>) {
    let service_name = env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| {
        if cfg!(feature = "airbender") {
            "ere-server-airbender"
        } else if cfg!(feature = "openvm") {
            "ere-server-openvm"
        } else if cfg!(feature = "risc0") {
            "ere-server-risc0"
        } else if cfg!(feature = "sp1") {
            "ere-server-sp1"
        } else if cfg!(feature = "zisk") {
            "ere-server-zisk"
        } else {
            unreachable!()
        }
        .to_owned()
    });

    let otel_endpoint = env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();

    let provider = otel_endpoint.map(|endpoint| {
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
        let exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .expect("failed to create OTLP exporter");
        let resource = Resource::builder()
            .with_service_name(service_name.clone())
            .build();
        SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource)
            .build()
    });

    let otel_layer = provider
        .as_ref()
        .map(|p| OpenTelemetryLayer::new(p.tracer(service_name)));

    (provider, otel_layer)
}

pub fn trace_layer_make_span(req: &Request) -> Span {
    struct OtelExtractor<'a>(&'a HeaderMap);

    impl Extractor for OtelExtractor<'_> {
        fn get(&self, key: &str) -> Option<&str> {
            self.0.get(key).and_then(|value| value.to_str().ok())
        }

        fn keys(&self) -> Vec<&str> {
            self.0.keys().map(|key| key.as_str()).collect()
        }
    }

    let method = path_to_method(req.uri().path());
    let span = info_span!("request", method, status = Empty, cancelled = Empty);
    let parent = opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.extract(&OtelExtractor(req.headers()))
    });
    let _ = span.set_parent(parent);
    span
}

pub fn trace_layer_on_response(res: &Response, latency: Duration, span: &Span) {
    let status = res.status().as_u16();
    span.record("status", status);
    span.record("cancelled", false);
    match status {
        500.. => error!(?latency, "internal error"),
        400..500 => warn!(?latency, "client error"),
        _ => info!(?latency, "ok"),
    }
}

pub fn trace_layer_on_failure(error: ServerErrorsFailureClass, latency: Duration, _span: &Span) {
    if let ServerErrorsFailureClass::Error(ref error) = error {
        error!(?latency, %error, "connection error");
    }
}

/// Layer that records the wall-clock duration and `cancelled=true` on the surrounding
/// `TraceLayer` span when the request future is dropped before completing.
///
/// Works around a design choice in `tracing-opentelemetry`, `on_exit` unconditionally sets
/// `end_time = now()` on every span exit, and `on_close` finalizes the exported OTel span using
/// that last-recorded value. A handler that yields `Pending` once and is then cancelled (client
/// disconnect, tokio timeout, etc.) is dropped without a second poll, so `end_time` stays frozen at
/// the first-poll moment and the exported span reports microseconds of duration instead of
/// wall-clock. On `Drop` this layer enters the captured span once more, firing a final `on_exit`
/// that bumps `end_time` to now, and records `cancelled=true` so Tempo/Jaeger queries can
/// distinguish cancellations from completed requests.
///
/// Must be installed inside the `TraceLayer`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RecordCancellationLayer;

impl<S> Layer<S> for RecordCancellationLayer {
    type Service = RecordCancellation<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RecordCancellation { inner }
    }
}

#[derive(Clone, Debug)]
pub struct RecordCancellation<S> {
    inner: S,
}

impl<S, ReqBody> Service<Request<ReqBody>> for RecordCancellation<S>
where
    S: Service<Request<ReqBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = RecordCancellationFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        RecordCancellationFuture {
            inner: self.inner.call(req),
            span: Some(Span::current()),
        }
    }
}

pin_project! {
    pub struct RecordCancellationFuture<F> {
        #[pin]
        inner: F,
        // `Some` while the request is in flight; taken on successful completion so `Drop` is a
        // no-op on the happy path (TraceLayer's own on_exit already set end_time correctly there).
        span: Option<Span>,
    }

    impl<F> PinnedDrop for RecordCancellationFuture<F> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            if let Some(span) = this.span.take() {
                span.record("cancelled", true);
                let _entered = span.enter();
            }
        }
    }
}

impl<F: Future> Future for RecordCancellationFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let output = ready!(this.inner.poll(cx));
        *this.span = None;
        Poll::Ready(output)
    }
}
