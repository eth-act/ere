use crate::api::{
    ExecuteRequest, ProveRequest, VerifyRequest, ZkvmService,
    execute_response::Result as ExecuteResult, prove_response::Result as ProveResult,
    verify_response::Result as VerifyResult,
};
use ere_prover_core::prover::{Input, ProgramExecutionReport, ProgramProvingReport, PublicValues};
use std::time::Duration;
use thiserror::Error;
use twirp::{Client, Middleware, Request, reqwest};

pub use twirp::{
    TwirpErrorResponse,
    url::{ParseError, Url},
};

#[cfg(feature = "otel")]
pub use otel_propagation::OtelPropagation;

const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Error)]
#[allow(non_camel_case_types)]
pub enum Error {
    #[error("Invalid URL: {0}")]
    ParseUrl(#[from] ParseError),
    #[error("zkVMProver method error: {0}")]
    zkVMProver(String),
    #[error("RPC error: {0}")]
    Rpc(#[from] TwirpErrorResponse),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedProof(pub Vec<u8>);

/// zkVMProver client of the `zkVMServer`.
#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub struct zkVMClient {
    endpoint: Url,
    http_client: reqwest::Client,
    client: Client,
}

impl zkVMClient {
    pub fn new(
        endpoint: Url,
        http_client: reqwest::Client,
        middlewares: Vec<Box<dyn Middleware>>,
    ) -> Result<Self, Error> {
        let client = Client::new(
            endpoint.join("twirp")?,
            http_client.clone(),
            middlewares,
            None,
        );
        Ok(Self {
            endpoint,
            http_client,
            client,
        })
    }

    pub fn from_endpoint(endpoint: Url) -> Result<Self, Error> {
        Self::new(endpoint, reqwest::Client::new(), vec![])
    }

    pub async fn is_healthy(&self) -> bool {
        let Ok(url) = self.endpoint.join("health") else {
            return false;
        };
        self.http_client
            .get(url)
            .timeout(HEALTH_CHECK_TIMEOUT)
            .send()
            .await
            .is_ok_and(|r| r.status().is_success())
    }

    pub async fn execute(
        &self,
        input: Input,
    ) -> Result<(PublicValues, ProgramExecutionReport), Error> {
        let request = Request::new(ExecuteRequest {
            input_stdin: input.stdin,
            input_proofs: input.proofs,
        });

        let response = self.client.execute(request).await?;

        match response.into_body().result.ok_or_else(result_none_err)? {
            ExecuteResult::Ok(result) => Ok((
                result.public_values.into(),
                bincode::serde::decode_from_slice(&result.report, bincode::config::legacy())
                    .map_err(deserialize_report_err)?
                    .0,
            )),
            ExecuteResult::Err(err) => Err(Error::zkVMProver(err)),
        }
    }

    pub async fn prove(
        &self,
        input: Input,
    ) -> Result<(PublicValues, EncodedProof, ProgramProvingReport), Error> {
        let request = Request::new(ProveRequest {
            input_stdin: input.stdin,
            input_proofs: input.proofs,
        });

        let response = self.client.prove(request).await?;

        match response.into_body().result.ok_or_else(result_none_err)? {
            ProveResult::Ok(result) => Ok((
                result.public_values.into(),
                EncodedProof(result.proof),
                bincode::serde::decode_from_slice(&result.report, bincode::config::legacy())
                    .map_err(deserialize_report_err)?
                    .0,
            )),
            ProveResult::Err(err) => Err(Error::zkVMProver(err)),
        }
    }

    pub async fn verify(&self, proof: EncodedProof) -> Result<PublicValues, Error> {
        let request = Request::new(VerifyRequest { proof: proof.0 });

        let response = self.client.verify(request).await?;

        match response.into_body().result.ok_or_else(result_none_err)? {
            VerifyResult::Ok(result) => Ok(result.public_values.into()),
            VerifyResult::Err(err) => Err(Error::zkVMProver(err)),
        }
    }
}

fn result_none_err() -> TwirpErrorResponse {
    twirp::internal("response result should always be Some")
}

fn deserialize_report_err(err: bincode::error::DecodeError) -> TwirpErrorResponse {
    twirp::internal(format!("failed to deserialize report: {err}"))
}

#[cfg(feature = "otel")]
mod otel_propagation {
    use tracing_opentelemetry::OpenTelemetrySpanExt;
    use twirp::{
        Middleware, Next,
        axum::http::{HeaderMap, HeaderName, HeaderValue},
        reqwest,
    };

    struct OtelInjector<'a>(&'a mut HeaderMap);

    impl opentelemetry::propagation::Injector for OtelInjector<'_> {
        fn set(&mut self, key: &str, value: String) {
            if let Ok(name) = HeaderName::from_bytes(key.as_bytes())
                && let Ok(val) = HeaderValue::from_str(&value)
            {
                self.0.insert(name, val);
            }
        }
    }

    pub struct OtelPropagation;

    #[twirp::async_trait::async_trait]
    impl Middleware for OtelPropagation {
        async fn handle(
            &self,
            mut req: reqwest::Request,
            next: Next<'_>,
        ) -> twirp::Result<reqwest::Response> {
            let context = tracing::Span::current().context();
            opentelemetry::global::get_text_map_propagator(|propagator| {
                propagator.inject_context(&context, &mut OtelInjector(req.headers_mut()));
            });
            next.run(req).await
        }
    }
}
