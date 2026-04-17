use core::time::Duration;
use opentelemetry::{propagation::Extractor, trace::TracerProvider};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{SdkTracer, SdkTracerProvider},
};
use std::env;
use tower_http::{classify::ServerErrorsFailureClass, trace::TraceLayer};
use tracing::{Span, error, info, info_span, warn};
use tracing_opentelemetry::{OpenTelemetryLayer, OpenTelemetrySpanExt};
use tracing_subscriber::Registry;
use twirp::{Request, Response, Router, axum::http::HeaderMap};

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

pub fn layer(app: Router) -> Router {
    app.layer(
        TraceLayer::new_for_http()
            .make_span_with(|req: &Request<_>| {
                let path = req.uri().path();
                let method = match path {
                    "/twirp/api.ZkvmService/Execute" => "execute",
                    "/twirp/api.ZkvmService/Prove" => "prove",
                    "/twirp/api.ZkvmService/Verify" => "verify",
                    _ => path,
                };
                let span = info_span!("request", method, status = tracing::field::Empty);
                let parent = opentelemetry::global::get_text_map_propagator(|propagator| {
                    propagator.extract(&OtelExtractor(req.headers()))
                });
                let _ = span.set_parent(parent);
                span
            })
            .on_request(())
            .on_response(|res: &Response<_>, latency: Duration, span: &Span| {
                let status = res.status().as_u16();
                span.record("status", status);
                match status {
                    500.. => error!(?latency, "internal error"),
                    400..500 => warn!(?latency, "client error"),
                    _ => info!(?latency, "ok"),
                }
            })
            .on_failure(
                |error: ServerErrorsFailureClass, latency: Duration, _span: &Span| {
                    if let ServerErrorsFailureClass::Error(ref error) = error {
                        error!(?latency, %error, "connection error");
                    }
                },
            ),
    )
}

struct OtelExtractor<'a>(&'a HeaderMap);

impl Extractor for OtelExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}
