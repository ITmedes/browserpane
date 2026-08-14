use std::fmt;

use thiserror::Error;
use url::Url;

const MAX_QUEUE_SIZE: usize = 8_192;
const MAX_BATCH_SIZE: usize = 1_024;
const MAX_SCHEDULE_DELAY_MS: u64 = 10_000;
const MAX_EXPORT_TIMEOUT_MS: u64 = 30_000;

#[derive(Clone, Eq, PartialEq)]
pub(crate) enum TraceExporter {
    None,
    OtlpGrpc { endpoint: Url },
}

impl fmt::Debug for TraceExporter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => formatter.write_str("None"),
            Self::OtlpGrpc { .. } => formatter.write_str("OtlpGrpc { endpoint: [redacted] }"),
        }
    }
}

#[derive(Clone, PartialEq)]
pub(crate) struct TelemetryConfig {
    pub(crate) exporter: TraceExporter,
    pub(crate) sampler: TraceSampler,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TraceSampler {
    AlwaysOn,
    AlwaysOff,
    ParentBasedAlwaysOn,
    ParentBasedAlwaysOff,
    TraceIdRatio(f64),
    ParentBasedTraceIdRatio(f64),
}

impl fmt::Debug for TelemetryConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TelemetryConfig")
            .field("exporter", &self.exporter)
            .field("sampler", &self.sampler)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum TelemetryConfigError {
    #[error("OTEL_SDK_DISABLED must be true or false")]
    InvalidSdkDisabled,
    #[error("OTEL_TRACES_EXPORTER must be none or otlp")]
    UnsupportedTraceExporter,
    #[error("OTLP trace export requires an explicit endpoint")]
    MissingOtlpEndpoint,
    #[error("the OTLP endpoint is not a safe gRPC endpoint")]
    InvalidOtlpEndpoint,
    #[error("only the grpc OTLP trace protocol is supported")]
    UnsupportedOtlpProtocol,
    #[error("the OpenTelemetry batch processor setting is outside its safety bound")]
    InvalidBatchSetting,
    #[error("the OpenTelemetry sampler setting is unsupported or invalid")]
    InvalidSampler,
}

impl TelemetryConfig {
    pub(crate) fn from_env() -> Result<Self, TelemetryConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, TelemetryConfigError> {
        if parse_disabled(lookup("OTEL_SDK_DISABLED"))? {
            return Ok(Self {
                exporter: TraceExporter::None,
                sampler: TraceSampler::ParentBasedAlwaysOn,
            });
        }
        validate_batch_settings(&mut lookup)?;
        let sampler = parse_sampler(&mut lookup)?;
        let exporter = match normalized(lookup("OTEL_TRACES_EXPORTER")).as_deref() {
            None | Some("none") => TraceExporter::None,
            Some("otlp") => {
                validate_protocol(&mut lookup)?;
                let endpoint = endpoint(&mut lookup)?;
                TraceExporter::OtlpGrpc { endpoint }
            }
            Some(_) => return Err(TelemetryConfigError::UnsupportedTraceExporter),
        };
        Ok(Self { exporter, sampler })
    }
}

fn parse_disabled(value: Option<String>) -> Result<bool, TelemetryConfigError> {
    match normalized(value).as_deref() {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        Some(_) => Err(TelemetryConfigError::InvalidSdkDisabled),
    }
}

fn validate_protocol(
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> Result<(), TelemetryConfigError> {
    let protocol = normalized(lookup("OTEL_EXPORTER_OTLP_TRACES_PROTOCOL"))
        .or_else(|| normalized(lookup("OTEL_EXPORTER_OTLP_PROTOCOL")));
    match protocol.as_deref() {
        None | Some("grpc") => Ok(()),
        Some(_) => Err(TelemetryConfigError::UnsupportedOtlpProtocol),
    }
}

fn endpoint(lookup: &mut impl FnMut(&str) -> Option<String>) -> Result<Url, TelemetryConfigError> {
    let value = trimmed(lookup("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT"))
        .or_else(|| trimmed(lookup("OTEL_EXPORTER_OTLP_ENDPOINT")))
        .ok_or(TelemetryConfigError::MissingOtlpEndpoint)?;
    let endpoint = Url::parse(&value).map_err(|_| TelemetryConfigError::InvalidOtlpEndpoint)?;
    if !matches!(endpoint.scheme(), "http" | "https")
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || !matches!(endpoint.path(), "" | "/")
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(TelemetryConfigError::InvalidOtlpEndpoint);
    }
    Ok(endpoint)
}

fn validate_batch_settings(
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> Result<(), TelemetryConfigError> {
    let queue = bounded_usize(lookup("OTEL_BSP_MAX_QUEUE_SIZE"), MAX_QUEUE_SIZE)?;
    let batch = bounded_usize(lookup("OTEL_BSP_MAX_EXPORT_BATCH_SIZE"), MAX_BATCH_SIZE)?;
    if matches!((queue, batch), (Some(queue), Some(batch)) if batch > queue) {
        return Err(TelemetryConfigError::InvalidBatchSetting);
    }
    bounded_u64(lookup("OTEL_BSP_SCHEDULE_DELAY"), MAX_SCHEDULE_DELAY_MS)?;
    bounded_u64(lookup("OTEL_BSP_EXPORT_TIMEOUT"), MAX_EXPORT_TIMEOUT_MS)?;
    Ok(())
}

fn parse_sampler(
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> Result<TraceSampler, TelemetryConfigError> {
    let sampler = normalized(lookup("OTEL_TRACES_SAMPLER"));
    let argument = trimmed(lookup("OTEL_TRACES_SAMPLER_ARG"));
    match sampler.as_deref() {
        None if argument.is_none() => Ok(TraceSampler::ParentBasedAlwaysOn),
        Some("always_on") if argument.is_none() => Ok(TraceSampler::AlwaysOn),
        Some("always_off") if argument.is_none() => Ok(TraceSampler::AlwaysOff),
        Some("parentbased_always_on") if argument.is_none() => {
            Ok(TraceSampler::ParentBasedAlwaysOn)
        }
        Some("parentbased_always_off") if argument.is_none() => {
            Ok(TraceSampler::ParentBasedAlwaysOff)
        }
        Some("traceidratio" | "parentbased_traceidratio") => {
            let ratio = argument
                .as_deref()
                .ok_or(TelemetryConfigError::InvalidSampler)?
                .parse::<f64>()
                .map_err(|_| TelemetryConfigError::InvalidSampler)?;
            if ratio.is_finite() && (0.0..=1.0).contains(&ratio) {
                if sampler.as_deref() == Some("traceidratio") {
                    Ok(TraceSampler::TraceIdRatio(ratio))
                } else {
                    Ok(TraceSampler::ParentBasedTraceIdRatio(ratio))
                }
            } else {
                Err(TelemetryConfigError::InvalidSampler)
            }
        }
        _ => Err(TelemetryConfigError::InvalidSampler),
    }
}

fn bounded_usize(
    value: Option<String>,
    maximum: usize,
) -> Result<Option<usize>, TelemetryConfigError> {
    trimmed(value)
        .map(|value| {
            let parsed = value
                .parse::<usize>()
                .map_err(|_| TelemetryConfigError::InvalidBatchSetting)?;
            if parsed == 0 || parsed > maximum {
                Err(TelemetryConfigError::InvalidBatchSetting)
            } else {
                Ok(parsed)
            }
        })
        .transpose()
}

fn bounded_u64(value: Option<String>, maximum: u64) -> Result<Option<u64>, TelemetryConfigError> {
    trimmed(value)
        .map(|value| {
            let parsed = value
                .parse::<u64>()
                .map_err(|_| TelemetryConfigError::InvalidBatchSetting)?;
            if parsed == 0 || parsed > maximum {
                Err(TelemetryConfigError::InvalidBatchSetting)
            } else {
                Ok(parsed)
            }
        })
        .transpose()
}

fn trimmed(value: Option<String>) -> Option<String> {
    value.map(|value| value.trim().to_string())
}

fn normalized(value: Option<String>) -> Option<String> {
    trimmed(value).map(|value| value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn config(values: &[(&str, &str)]) -> Result<TelemetryConfig, TelemetryConfigError> {
        let values = values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<HashMap<_, _>>();
        TelemetryConfig::from_lookup(|name| values.get(name).cloned())
    }

    #[test]
    fn disables_export_by_default_and_by_standard_sdk_switch() {
        assert_eq!(config(&[]).unwrap().exporter, TraceExporter::None);
        assert_eq!(
            config(&[]).unwrap().sampler,
            TraceSampler::ParentBasedAlwaysOn
        );
        assert_eq!(
            config(&[
                ("OTEL_SDK_DISABLED", "true"),
                ("OTEL_TRACES_EXPORTER", "otlp")
            ])
            .unwrap()
            .exporter,
            TraceExporter::None
        );
    }

    #[test]
    fn accepts_explicit_credential_free_grpc_endpoint() {
        let parsed = config(&[
            ("OTEL_TRACES_EXPORTER", "otlp"),
            (
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                "https://collector.example:4317",
            ),
            ("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc"),
            ("OTEL_TRACES_SAMPLER", "parentbased_traceidratio"),
            ("OTEL_TRACES_SAMPLER_ARG", "0.25"),
        ])
        .unwrap();
        assert!(matches!(parsed.exporter, TraceExporter::OtlpGrpc { .. }));
        assert_eq!(parsed.sampler, TraceSampler::ParentBasedTraceIdRatio(0.25));
    }

    #[test]
    fn rejects_missing_unsafe_and_unsupported_exporter_configuration() {
        assert_eq!(
            config(&[("OTEL_TRACES_EXPORTER", "otlp")]).unwrap_err(),
            TelemetryConfigError::MissingOtlpEndpoint
        );
        assert_eq!(
            config(&[
                ("OTEL_TRACES_EXPORTER", "otlp"),
                (
                    "OTEL_EXPORTER_OTLP_ENDPOINT",
                    "https://token@collector.example:4317"
                ),
            ])
            .unwrap_err(),
            TelemetryConfigError::InvalidOtlpEndpoint
        );
        assert_eq!(
            config(&[("OTEL_TRACES_EXPORTER", "jaeger")]).unwrap_err(),
            TelemetryConfigError::UnsupportedTraceExporter
        );
    }

    #[test]
    fn rejects_unbounded_batch_and_invalid_sampler_settings() {
        assert_eq!(
            config(&[("OTEL_BSP_MAX_QUEUE_SIZE", "8193")]).unwrap_err(),
            TelemetryConfigError::InvalidBatchSetting
        );
        assert_eq!(
            config(&[
                ("OTEL_BSP_MAX_QUEUE_SIZE", "16"),
                ("OTEL_BSP_MAX_EXPORT_BATCH_SIZE", "17"),
            ])
            .unwrap_err(),
            TelemetryConfigError::InvalidBatchSetting
        );
        assert_eq!(
            config(&[
                ("OTEL_TRACES_SAMPLER", "traceidratio"),
                ("OTEL_TRACES_SAMPLER_ARG", "2"),
            ])
            .unwrap_err(),
            TelemetryConfigError::InvalidSampler
        );
    }

    #[test]
    fn debug_and_errors_do_not_expose_endpoint_values() {
        let marker = "collector-sensitive-marker.example";
        let parsed = config(&[
            ("OTEL_TRACES_EXPORTER", "otlp"),
            (
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                &format!("https://{marker}:4317"),
            ),
        ])
        .unwrap();
        assert!(!format!("{parsed:?}").contains(marker));
        let error = config(&[
            ("OTEL_TRACES_EXPORTER", "otlp"),
            (
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                &format!("https://user@{marker}:4317"),
            ),
        ])
        .unwrap_err();
        assert!(!error.to_string().contains(marker));
        assert!(!format!("{error:?}").contains(marker));
    }
}
