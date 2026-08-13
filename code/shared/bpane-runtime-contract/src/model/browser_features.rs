use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use super::{ContractErrorCode, ContractViolation};

/// Browser runtime launch intent. Docker-sensitive fields are broker-owned.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserRuntimeLaunchRequest {
    /// Session that owns the runtime.
    pub session_id: Uuid,
    /// Optional reusable browser context mounted by broker policy.
    pub browser_context_id: Option<Uuid>,
    /// Approved browser features materialized by broker policy.
    #[serde(default, skip_serializing_if = "BrowserRuntimeFeatures::is_empty")]
    pub features: BrowserRuntimeFeatures,
}

/// Bounded browser feature selections. Runtime paths and Docker fields are
/// intentionally absent.
#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserRuntimeFeatures {
    /// Browser-visible locale, timezone, geolocation, and identity settings.
    #[serde(default, skip_serializing_if = "BrowserNetworkIdentity::is_empty")]
    pub network_identity: BrowserNetworkIdentity,
    /// Optional approved egress behavior.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub egress: Option<BrowserEgressSelection>,
    /// Approved extension versions resolved by trusted broker configuration.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extension_version_ids: Vec<Uuid>,
    /// Whether a session-file manifest has been prepared in session data.
    #[serde(default, skip_serializing_if = "is_false")]
    pub session_file_bindings: bool,
}

impl BrowserRuntimeFeatures {
    fn is_empty(&self) -> bool {
        self.network_identity.is_empty()
            && self.egress.is_none()
            && self.extension_version_ids.is_empty()
            && !self.session_file_bindings
    }

    pub(super) fn validate(&self) -> Result<(), ContractViolation> {
        self.network_identity.validate()?;
        if let Some(egress) = &self.egress {
            egress.validate()?;
        }
        if self.extension_version_ids.len() > 32 {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        let mut unique_ids = BTreeSet::new();
        for extension_id in &self.extension_version_ids {
            if extension_id.is_nil() || !unique_ids.insert(*extension_id) {
                return Err(ContractErrorCode::InvalidOperationParameters.into());
            }
        }
        Ok(())
    }
}

/// Browser-visible network identity values.
#[derive(Debug, Clone, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserNetworkIdentity {
    /// BCP 47-like locale used by Chromium and the POSIX environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// Ordered browser language preferences.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    /// IANA timezone identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Optional fixed-point browser geolocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geolocation: Option<BrowserGeolocation>,
    /// Optional Chromium user agent override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Optional application-defined browser identity label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser_identity: Option<String>,
}

impl BrowserNetworkIdentity {
    fn is_empty(&self) -> bool {
        self.locale.is_none()
            && self.languages.is_empty()
            && self.timezone.is_none()
            && self.geolocation.is_none()
            && self.user_agent.is_none()
            && self.browser_identity.is_none()
    }

    fn validate(&self) -> Result<(), ContractViolation> {
        if self
            .locale
            .as_deref()
            .is_some_and(|value| !is_locale(value))
            || self
                .timezone
                .as_deref()
                .is_some_and(|value| !is_timezone(value))
        {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        validate_optional_text(&self.user_agent, 1_024)?;
        validate_optional_text(&self.browser_identity, 128)?;
        if self.languages.len() > 16 {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        for language in &self.languages {
            if !is_locale(language) {
                return Err(ContractErrorCode::InvalidOperationParameters.into());
            }
        }
        if let Some(geolocation) = &self.geolocation {
            geolocation.validate()?;
        }
        Ok(())
    }
}

/// Fixed-point browser geolocation, avoiding floating-point ambiguity in
/// idempotency comparisons.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserGeolocation {
    /// Latitude multiplied by 10,000,000.
    pub latitude_e7: i32,
    /// Longitude multiplied by 10,000,000.
    pub longitude_e7: i32,
    /// Optional accuracy in millimeters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accuracy_mm: Option<u32>,
}

impl BrowserGeolocation {
    fn validate(&self) -> Result<(), ContractViolation> {
        if !(-900_000_000..=900_000_000).contains(&self.latitude_e7)
            || !(-1_800_000_000..=1_800_000_000).contains(&self.longitude_e7)
            || self
                .accuracy_mm
                .is_some_and(|value| value == 0 || value > 100_000_000)
        {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        Ok(())
    }
}

/// Egress traffic observation mode selected for a browser runtime.
#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserEgressObservationMode {
    /// Proxy metadata may be observed without decrypting HTTPS traffic.
    #[default]
    MetadataOnly,
    /// HTTPS traffic is intercepted by an explicitly configured proxy.
    TlsIntercept,
}

impl BrowserEgressObservationMode {
    /// Stable value used in broker-derived environment and labels.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MetadataOnly => "metadata_only",
            Self::TlsIntercept => "tls_intercept",
        }
    }
}

/// Marker for sensitive material prepared at a broker-owned session-data path.
#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserSessionDataSource {
    /// The later typed storage operation prepares the material in session data.
    SessionData,
}

/// Approved proxy selection without embedded credentials.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserProxySelection {
    /// Proxy URL without user information.
    pub url: String,
    /// Optional fixed-path proxy-auth material prerequisite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authentication: Option<BrowserSessionDataSource>,
}

/// Approved browser egress behavior.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserEgressSelection {
    /// Control-plane egress profile correlated with the runtime.
    pub profile_id: Uuid,
    /// Optional proxy used by Chromium.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<BrowserProxySelection>,
    /// Chromium proxy bypass rules.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bypass_rules: Vec<String>,
    /// Traffic observation mode.
    #[serde(default)]
    pub observation_mode: BrowserEgressObservationMode,
    /// Optional fixed-path trusted CA prerequisite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_ca: Option<BrowserSessionDataSource>,
    /// Whether an approved sensitive-log sink exists outside BrowserPane.
    #[serde(default, skip_serializing_if = "is_false")]
    pub sensitive_log_sink_configured: bool,
}

impl BrowserEgressSelection {
    fn validate(&self) -> Result<(), ContractViolation> {
        if self.profile_id.is_nil() || self.bypass_rules.len() > 128 {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        for rule in &self.bypass_rules {
            validate_text(rule, 512)?;
            if rule.contains(';') {
                return Err(ContractErrorCode::InvalidOperationParameters.into());
            }
        }
        if let Some(proxy) = &self.proxy {
            validate_proxy_url(&proxy.url)?;
        }
        let intercepting = self.observation_mode == BrowserEgressObservationMode::TlsIntercept;
        if (intercepting
            && (self.proxy.is_none()
                || self.custom_ca.is_none()
                || !self.sensitive_log_sink_configured))
            || (!intercepting && self.custom_ca.is_some())
        {
            return Err(ContractErrorCode::InvalidOperationParameters.into());
        }
        Ok(())
    }
}

fn validate_optional_text(
    value: &Option<String>,
    maximum_bytes: usize,
) -> Result<(), ContractViolation> {
    if let Some(value) = value {
        validate_text(value, maximum_bytes)?;
    }
    Ok(())
}

fn validate_text(value: &str, maximum_bytes: usize) -> Result<(), ContractViolation> {
    if value.is_empty()
        || value.len() > maximum_bytes
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(ContractErrorCode::InvalidOperationParameters.into());
    }
    Ok(())
}

fn validate_proxy_url(value: &str) -> Result<(), ContractViolation> {
    validate_text(value, 2_048)?;
    if value.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(ContractErrorCode::InvalidOperationParameters.into());
    }
    let parsed = Url::parse(value)
        .map_err(|_| ContractViolation::from(ContractErrorCode::InvalidOperationParameters))?;
    let valid_scheme = matches!(parsed.scheme(), "http" | "https");
    let valid_path = parsed.path().is_empty() || parsed.path() == "/";
    if !valid_scheme
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !valid_path
    {
        return Err(ContractErrorCode::InvalidOperationParameters.into());
    }
    Ok(())
}

fn is_false(value: &bool) -> bool {
    !value
}

fn is_locale(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .split('-')
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_alphanumeric()))
}

fn is_timezone(value: &str) -> bool {
    value == "UTC"
        || (!value.is_empty()
            && value.len() <= 128
            && !value.starts_with('/')
            && !value.ends_with('/')
            && !value.contains("..")
            && value.contains('/')
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'+')
            }))
}
