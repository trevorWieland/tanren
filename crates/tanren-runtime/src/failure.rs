mod taxonomy;
mod text_classifier;

use serde::{Deserialize, Serialize};
use tanren_domain::ErrorClass;
use taxonomy::{classify_exact_identifier, classify_exit_code};

/// Canonical failure classes exposed by harness adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessFailureClass {
    CapabilityDenied,
    ApprovalDenied,
    Authentication,
    RateLimited,
    Timeout,
    TransportUnavailable,
    ResourceExhausted,
    InvalidRequest,
    Fatal,
    Transient,
    Unknown,
}

impl HarnessFailureClass {
    /// Map harness failure class into the domain retry class.
    #[must_use]
    pub const fn to_domain_error_class(self) -> ErrorClass {
        match self {
            Self::RateLimited
            | Self::Timeout
            | Self::TransportUnavailable
            | Self::ResourceExhausted
            | Self::Transient => ErrorClass::Transient,
            Self::CapabilityDenied
            | Self::ApprovalDenied
            | Self::Authentication
            | Self::InvalidRequest
            | Self::Fatal => ErrorClass::Fatal,
            Self::Unknown => ErrorClass::Ambiguous,
        }
    }
}

/// Typed provider-level error codes adapters emit for terminal failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureCode {
    CapabilityDenied,
    ApprovalDenied,
    Authentication,
    RateLimited,
    Timeout,
    TransportUnavailable,
    ResourceExhausted,
    InvalidRequest,
    Fatal,
    Transient,
}

impl ProviderFailureCode {
    #[must_use]
    pub const fn to_harness_failure_class(self) -> HarnessFailureClass {
        match self {
            Self::CapabilityDenied => HarnessFailureClass::CapabilityDenied,
            Self::ApprovalDenied => HarnessFailureClass::ApprovalDenied,
            Self::Authentication => HarnessFailureClass::Authentication,
            Self::RateLimited => HarnessFailureClass::RateLimited,
            Self::Timeout => HarnessFailureClass::Timeout,
            Self::TransportUnavailable => HarnessFailureClass::TransportUnavailable,
            Self::ResourceExhausted => HarnessFailureClass::ResourceExhausted,
            Self::InvalidRequest => HarnessFailureClass::InvalidRequest,
            Self::Fatal => HarnessFailureClass::Fatal,
            Self::Transient => HarnessFailureClass::Transient,
        }
    }
}

/// Typed provider-level error codes used in audit/telemetry paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuditProviderFailureCode {
    CapabilityDenied,
    ApprovalDenied,
    Authentication,
    RateLimited,
    Timeout,
    TransportUnavailable,
    ResourceExhausted,
    InvalidRequest,
    Fatal,
    Transient,
    #[default]
    Unknown,
}

impl AuditProviderFailureCode {
    #[must_use]
    pub const fn to_terminal_code(self) -> Option<ProviderFailureCode> {
        match self {
            Self::CapabilityDenied => Some(ProviderFailureCode::CapabilityDenied),
            Self::ApprovalDenied => Some(ProviderFailureCode::ApprovalDenied),
            Self::Authentication => Some(ProviderFailureCode::Authentication),
            Self::RateLimited => Some(ProviderFailureCode::RateLimited),
            Self::Timeout => Some(ProviderFailureCode::Timeout),
            Self::TransportUnavailable => Some(ProviderFailureCode::TransportUnavailable),
            Self::ResourceExhausted => Some(ProviderFailureCode::ResourceExhausted),
            Self::InvalidRequest => Some(ProviderFailureCode::InvalidRequest),
            Self::Fatal => Some(ProviderFailureCode::Fatal),
            Self::Transient => Some(ProviderFailureCode::Transient),
            Self::Unknown => None,
        }
    }
}

impl From<ProviderFailureCode> for AuditProviderFailureCode {
    fn from(value: ProviderFailureCode) -> Self {
        match value {
            ProviderFailureCode::CapabilityDenied => Self::CapabilityDenied,
            ProviderFailureCode::ApprovalDenied => Self::ApprovalDenied,
            ProviderFailureCode::Authentication => Self::Authentication,
            ProviderFailureCode::RateLimited => Self::RateLimited,
            ProviderFailureCode::Timeout => Self::Timeout,
            ProviderFailureCode::TransportUnavailable => Self::TransportUnavailable,
            ProviderFailureCode::ResourceExhausted => Self::ResourceExhausted,
            ProviderFailureCode::InvalidRequest => Self::InvalidRequest,
            ProviderFailureCode::Fatal => Self::Fatal,
            ProviderFailureCode::Transient => Self::Transient,
        }
    }
}

/// Normalized provider identifier used by failure classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProviderIdentifier(String);

impl ProviderIdentifier {
    pub const MAX_LEN: usize = 64;

    /// Build a strict provider identifier.
    ///
    /// # Errors
    /// Returns [`ProviderIdentifierError`] when empty or format-invalid.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ProviderIdentifierError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ProviderIdentifierError::EmptyOrWhitespace);
        }
        if value.len() > Self::MAX_LEN {
            return Err(ProviderIdentifierError::TooLong {
                max_len: Self::MAX_LEN,
            });
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
        {
            return Err(ProviderIdentifierError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for ProviderIdentifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::try_new(raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderIdentifierError {
    #[error("provider identifier must not be empty")]
    EmptyOrWhitespace,
    #[error("provider identifier exceeds max length {max_len}")]
    TooLong { max_len: usize },
    #[error("provider identifier contains unsupported characters")]
    InvalidCharacter,
}

/// Normalized provider run identifier surfaced on successful execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProviderRunId(String);

impl ProviderRunId {
    pub const MAX_LEN: usize = 128;

    /// Build a strict provider run identifier.
    ///
    /// # Errors
    /// Returns [`ProviderRunIdError`] when empty or format-invalid.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ProviderRunIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ProviderRunIdError::EmptyOrWhitespace);
        }
        if value.len() > Self::MAX_LEN {
            return Err(ProviderRunIdError::TooLong {
                max_len: Self::MAX_LEN,
            });
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
        {
            return Err(ProviderRunIdError::InvalidCharacter);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl<'de> Deserialize<'de> for ProviderRunId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::try_new(raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderRunIdError {
    #[error("provider run id must not be empty")]
    EmptyOrWhitespace,
    #[error("provider run id exceeds max length {max_len}")]
    TooLong { max_len: usize },
    #[error("provider run id contains unsupported characters")]
    InvalidCharacter,
}

/// Normalized context adapters pass for terminal failure classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderFailureContext {
    pub typed_code: ProviderFailureCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<ProviderIdentifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<ProviderIdentifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_tail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_tail: Option<String>,
}

impl ProviderFailureContext {
    #[must_use]
    pub fn new(typed_code: ProviderFailureCode) -> Self {
        Self {
            typed_code,
            provider_code: None,
            provider_kind: None,
            signal: None,
            exit_code: None,
            stdout_tail: None,
            stderr_tail: None,
        }
    }
}

/// Context for audit-only fallback classification paths.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AuditProviderFailureContext {
    pub typed_code: AuditProviderFailureCode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<ProviderIdentifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<ProviderIdentifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_tail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_tail: Option<String>,
}

impl From<&ProviderFailureContext> for AuditProviderFailureContext {
    fn from(value: &ProviderFailureContext) -> Self {
        Self {
            typed_code: value.typed_code.into(),
            provider_code: value.provider_code.clone(),
            provider_kind: value.provider_kind.clone(),
            signal: value.signal.clone(),
            exit_code: value.exit_code,
            stdout_tail: value.stdout_tail.clone(),
            stderr_tail: value.stderr_tail.clone(),
        }
    }
}

/// Provider-native failure payload adapters return to the contract wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct ProviderFailure {
    pub message: String,
    #[serde(flatten)]
    pub context: ProviderFailureContext,
}

impl ProviderFailure {
    #[must_use]
    pub fn new(typed_code: ProviderFailureCode, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            context: ProviderFailureContext::new(typed_code),
        }
    }

    #[must_use]
    pub fn with_context(mut self, context: ProviderFailureContext) -> Self {
        self.context = context;
        self
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn into_harness_failure(self) -> HarnessFailure {
        HarnessFailure::from_provider_failure(self)
    }
}

/// Typed failure payload returned by the harness contract wrapper.
///
/// External callers cannot construct terminal failures directly; adapter
/// failures must be normalized by the contract wrapper.
///
/// ```compile_fail
/// use tanren_runtime::{HarnessFailure, ProviderFailureCode};
///
/// let _ = HarnessFailure::new(ProviderFailureCode::Fatal, "unsanitized");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[error("{class:?}: {message}")]
pub struct HarnessFailure {
    class: HarnessFailureClass,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_code: Option<ProviderIdentifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_kind: Option<ProviderIdentifier>,
    typed_code: ProviderFailureCode,
}

impl HarnessFailure {
    #[must_use]
    pub(crate) fn from_provider_failure(failure: ProviderFailure) -> Self {
        let class = classify_provider_failure(&failure.context);
        Self {
            class,
            message: failure.message,
            provider_code: failure.context.provider_code,
            provider_kind: failure.context.provider_kind,
            typed_code: failure.context.typed_code,
        }
    }

    #[must_use]
    pub const fn class(&self) -> HarnessFailureClass {
        self.class
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn provider_code(&self) -> Option<&ProviderIdentifier> {
        self.provider_code.as_ref()
    }

    #[must_use]
    pub fn provider_kind(&self) -> Option<&ProviderIdentifier> {
        self.provider_kind.as_ref()
    }

    #[must_use]
    pub const fn typed_code(&self) -> ProviderFailureCode {
        self.typed_code
    }
}

#[derive(Deserialize)]
struct HarnessFailureWire {
    class: HarnessFailureClass,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_code: Option<ProviderIdentifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider_kind: Option<ProviderIdentifier>,
    typed_code: ProviderFailureCode,
}

impl<'de> Deserialize<'de> for HarnessFailure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = HarnessFailureWire::deserialize(deserializer)?;
        let expected = wire.typed_code.to_harness_failure_class();
        if wire.class != expected {
            return Err(serde::de::Error::custom(format!(
                "typed_code {:?} implies class {expected:?}, got {:?}",
                wire.typed_code, wire.class
            )));
        }

        Ok(Self {
            class: wire.class,
            message: wire.message,
            provider_code: wire.provider_code,
            provider_kind: wire.provider_kind,
            typed_code: wire.typed_code,
        })
    }
}

/// Classify provider-native failures into stable harness classes.
///
/// Terminal semantic normalization is strictly typed and deterministic.
#[must_use]
pub fn classify_provider_failure(ctx: &ProviderFailureContext) -> HarnessFailureClass {
    ctx.typed_code.to_harness_failure_class()
}

/// Last-resort telemetry/audit classifier that allows bounded heuristics.
///
/// This API is explicitly non-authoritative for terminal failure semantics.
#[must_use]
pub fn classify_provider_failure_for_audit(
    ctx: &AuditProviderFailureContext,
) -> HarnessFailureClass {
    if let Some(typed_code) = ctx.typed_code.to_terminal_code() {
        return typed_code.to_harness_failure_class();
    }

    if let Some(class) = ctx
        .provider_code
        .as_ref()
        .and_then(|value| classify_exact_identifier(value.as_str()))
    {
        return class;
    }

    if let Some(class) = ctx
        .provider_kind
        .as_ref()
        .and_then(|value| classify_exact_identifier(value.as_str()))
    {
        return class;
    }

    if let Some(class) = ctx.signal.as_deref().and_then(classify_exact_identifier) {
        return class;
    }

    if let Some(class) = ctx.exit_code.and_then(classify_exit_code) {
        return class;
    }

    text_classifier::classify_text_fallback(ctx.stdout_tail.as_deref(), ctx.stderr_tail.as_deref())
}

#[cfg(test)]
mod tests;
