//! Bounded OpenTelemetry initialization and W3C Trace Context propagation.
//!
//! BrowserPane services use this crate to avoid service-specific propagation
//! formats and divergent exporter configuration. Trace export is disabled by
//! default and must be enabled explicitly through standard `OTEL_*` variables.

#![forbid(unsafe_code)]

mod config;

use http::HeaderMap;
use opentelemetry::global;
use opentelemetry::trace::{TraceContextExt as _, TracerProvider as _};
use opentelemetry::Context;
use opentelemetry_http::{HeaderExtractor, HeaderInjector};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use thiserror::Error;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

pub use config::TelemetryConfigError;
use config::{TelemetryConfig, TraceExporter};

#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum TelemetryInitError {
    #[error(transparent)]
    Configuration(#[from] TelemetryConfigError),
    #[error("the OTLP trace exporter could not be configured")]
    ExporterConfiguration,
    #[error("the tracing subscriber could not be installed")]
    SubscriberInstallation,
}

/// Owns the optional SDK provider and flushes it during shutdown.
#[derive(Debug)]
pub struct TelemetryGuard {
    provider: Option<SdkTracerProvider>,
}

impl TelemetryGuard {
    /// Flushes and shuts down the active trace provider.
    ///
    /// # Errors
    ///
    /// Returns a redacted error if the provider cannot finish shutdown.
    pub fn shutdown(mut self) -> Result<(), TelemetryShutdownError> {
        let Some(provider) = self.provider.take() else {
            return Ok(());
        };
        provider.shutdown().map_err(|_| TelemetryShutdownError)
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            let _ = provider.shutdown();
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
#[error("the OpenTelemetry trace provider could not finish shutdown")]
pub struct TelemetryShutdownError;

/// Installs formatted logging and optional OTLP tracing for one service.
///
/// # Errors
///
/// Returns a redacted error for invalid standard environment configuration,
/// exporter construction failure, or a tracing subscriber that is already set.
pub fn init(
    service_name: &'static str,
    default_filter: &'static str,
) -> Result<TelemetryGuard, TelemetryInitError> {
    let config = TelemetryConfig::from_env()?;
    global::set_text_map_propagator(TraceContextPropagator::new());
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));
    match config.exporter {
        TraceExporter::None => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer())
                .try_init()
                .map_err(|_| TelemetryInitError::SubscriberInstallation)?;
            Ok(TelemetryGuard { provider: None })
        }
        TraceExporter::OtlpGrpc { endpoint } => {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(endpoint.to_string())
                .build()
                .map_err(|_| TelemetryInitError::ExporterConfiguration)?;
            let provider = SdkTracerProvider::builder()
                .with_batch_exporter(exporter)
                .with_resource(
                    Resource::builder_empty()
                        .with_service_name(service_name)
                        .with_attribute(opentelemetry::KeyValue::new(
                            "service.version",
                            env!("CARGO_PKG_VERSION"),
                        ))
                        .build(),
                )
                .build();
            let tracer = provider.tracer(service_name);
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer())
                .with(tracing_opentelemetry::layer().with_tracer(tracer))
                .try_init()
                .map_err(|_| TelemetryInitError::SubscriberInstallation)?;
            Ok(TelemetryGuard {
                provider: Some(provider),
            })
        }
    }
}

/// Extracts standard remote trace context from HTTP headers.
pub fn extract_context(headers: &HeaderMap) -> Context {
    global::get_text_map_propagator(|propagator| propagator.extract(&HeaderExtractor(headers)))
}

/// Injects one explicit context into HTTP headers.
pub fn inject_context(context: &Context, headers: &mut HeaderMap) {
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(context, &mut HeaderInjector(headers));
    });
}

/// Injects the current `tracing` span context into HTTP headers.
pub fn inject_current_context(headers: &mut HeaderMap) {
    let span_context = tracing::Span::current().context();
    if span_context.span().span_context().is_valid() {
        inject_context(&span_context, headers);
    } else {
        inject_context(&Context::current(), headers);
    }
}

/// Sets the current tracing span's parent from standard HTTP trace headers.
pub fn set_parent_from_headers(span: &tracing::Span, headers: &HeaderMap) {
    let _ = span.set_parent(extract_context(headers));
}

#[cfg(test)]
mod tests {
    use opentelemetry::trace::TraceContextExt;
    use opentelemetry::trace::{SpanContext, SpanId, TraceFlags, TraceId, TraceState};

    use super::*;

    fn install_propagator() {
        global::set_text_map_propagator(TraceContextPropagator::new());
    }

    #[test]
    fn injects_and_extracts_w3c_trace_context() {
        install_propagator();
        let span_context = SpanContext::new(
            TraceId::from_hex("4bf92f3577b34da6a3ce929d0e0e4736").unwrap(),
            SpanId::from_hex("00f067aa0ba902b7").unwrap(),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        );
        let context = Context::new().with_remote_span_context(span_context);
        let mut headers = HeaderMap::new();

        inject_context(&context, &mut headers);
        assert_eq!(
            headers.get("traceparent").unwrap(),
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01"
        );
        let extracted = extract_context(&headers);
        assert_eq!(
            extracted.span().span_context().trace_id(),
            TraceId::from_hex("4bf92f3577b34da6a3ce929d0e0e4736").unwrap()
        );
    }

    #[test]
    fn malformed_trace_context_is_ignored() {
        install_propagator();
        let mut headers = HeaderMap::new();
        headers.insert("traceparent", "not-a-trace".parse().unwrap());
        headers.insert("tracestate", "sensitive=marker".parse().unwrap());

        let extracted = extract_context(&headers);
        assert!(!extracted.span().span_context().is_valid());
        let mut forwarded = HeaderMap::new();
        inject_context(&extracted, &mut forwarded);
        assert!(!forwarded.contains_key("traceparent"));
        assert!(!forwarded.contains_key("tracestate"));
    }
}
