mod taxonomy;
mod text_classifier;

use serde::{Deserialize, Serialize};
use tanren_domain::{ErrorClass, NonEmptyString};
use taxonomy::{classify_exact_identifier, classify_exit_code, normalize_identifier};

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

/// Typed provider-level error codes adapters should emit when available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
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
    #[default]
    Unknown,
}

impl ProviderFailureCode {
    #[must_use]
    pub const fn from_harness_failure_class(class: HarnessFailureClass) -> Self {
        match class {
            HarnessFailureClass::CapabilityDenied => Self::CapabilityDenied,
            HarnessFailureClass::ApprovalDenied => Self::ApprovalDenied,
            HarnessFailureClass::Authentication => Self::Authentication,
            HarnessFailureClass::RateLimited => Self::RateLimited,
            HarnessFailureClass::Timeout => Self::Timeout,
            HarnessFailureClass::TransportUnavailable => Self::TransportUnavailable,
            HarnessFailureClass::ResourceExhausted => Self::ResourceExhausted,
            HarnessFailureClass::InvalidRequest => Self::InvalidRequest,
            HarnessFailureClass::Fatal => Self::Fatal,
            HarnessFailureClass::Transient => Self::Transient,
            HarnessFailureClass::Unknown => Self::Unknown,
        }
    }

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
            Self::Unknown => HarnessFailureClass::Unknown,
        }
    }
}

/// Normalized provider identifier used by failure classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProviderIdentifier(NonEmptyString);

impl ProviderIdentifier {
    /// Build a normalized provider identifier.
    ///
    /// # Errors
    /// Returns [`ProviderIdentifierError`] when the value is blank.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ProviderIdentifierError> {
        let normalized = normalize_identifier(&value.into());
        let value = NonEmptyString::try_new(normalized)
            .map_err(|_| ProviderIdentifierError::EmptyOrWhitespace)?;
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
}

/// Normalized raw context adapters can pass into classification helpers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
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
            context: ProviderFailureContext {
                typed_code,
                ..ProviderFailureContext::default()
            },
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
        let class = classify_provider_failure(&self.context);
        HarnessFailure::from_provider_failure_with_class(self, class)
    }
}

/// Typed failure payload returned by the harness contract wrapper.
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
    pub fn new(class: HarnessFailureClass, message: impl Into<String>) -> Self {
        Self {
            class,
            message: message.into(),
            provider_code: None,
            provider_kind: None,
            typed_code: ProviderFailureCode::from_harness_failure_class(class),
        }
    }

    #[must_use]
    pub(crate) fn from_provider_failure_with_class(
        failure: ProviderFailure,
        class: HarnessFailureClass,
    ) -> Self {
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    typed_code: ProviderFailureCode,
}

impl<'de> Deserialize<'de> for HarnessFailure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = HarnessFailureWire::deserialize(deserializer)?;
        if wire.typed_code == ProviderFailureCode::Unknown {
            return Err(serde::de::Error::custom(
                "typed_code unknown is forbidden for terminal harness failures",
            ));
        }
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
pub fn classify_provider_failure_for_audit(ctx: &ProviderFailureContext) -> HarnessFailureClass {
    if ctx.typed_code != ProviderFailureCode::Unknown {
        return ctx.typed_code.to_harness_failure_class();
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

/// Validate terminal adapter failure context invariants.
///
/// # Errors
/// Returns [`TerminalFailureCodeError`] when `typed_code` is not admissible.
pub(crate) fn ensure_terminal_failure_code(
    code: ProviderFailureCode,
) -> Result<(), TerminalFailureCodeError> {
    if code == ProviderFailureCode::Unknown {
        return Err(TerminalFailureCodeError::UnknownForbidden);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TerminalFailureCodeError {
    #[error("typed_code unknown is forbidden for terminal adapter failures")]
    UnknownForbidden,
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn typed_code_is_the_only_terminal_classification_input() {
        let class = classify_provider_failure(&ProviderFailureContext {
            typed_code: ProviderFailureCode::Timeout,
            provider_code: Some(ProviderIdentifier::try_new("rate_limited").expect("code")),
            stderr_tail: Some("401 invalid api key".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::Timeout);
    }

    #[test]
    fn provider_failure_normalizes_through_context_classification() {
        let provider_failure =
            ProviderFailure::new(ProviderFailureCode::RateLimited, "raw adapter failure")
                .with_context(ProviderFailureContext {
                    typed_code: ProviderFailureCode::RateLimited,
                    stderr_tail: Some("fatal panic".into()),
                    ..ProviderFailureContext::default()
                });

        let failure = provider_failure.into_harness_failure();
        assert_eq!(failure.class(), HarnessFailureClass::RateLimited);
        assert_eq!(failure.typed_code(), ProviderFailureCode::RateLimited);
    }

    #[test]
    fn audit_classifier_uses_structured_and_text_fallback_only_for_unknown_typed_code() {
        let class = classify_provider_failure_for_audit(&ProviderFailureContext {
            typed_code: ProviderFailureCode::Unknown,
            provider_code: Some(ProviderIdentifier::try_new("rate_limited").expect("code")),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::RateLimited);

        let from_text = classify_provider_failure_for_audit(&ProviderFailureContext {
            typed_code: ProviderFailureCode::Unknown,
            stderr_tail: Some("429 too many requests".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(from_text, HarnessFailureClass::RateLimited);
    }

    #[test]
    fn maps_rate_limit_to_transient_domain_error_class() {
        let class = classify_provider_failure(&ProviderFailureContext {
            typed_code: ProviderFailureCode::RateLimited,
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::RateLimited);
        assert_eq!(class.to_domain_error_class(), ErrorClass::Transient);
    }

    #[test]
    fn maps_capability_denial_to_fatal_domain_error_class() {
        let class = classify_provider_failure(&ProviderFailureContext {
            typed_code: ProviderFailureCode::CapabilityDenied,
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::CapabilityDenied);
        assert_eq!(class.to_domain_error_class(), ErrorClass::Fatal);
    }

    #[test]
    fn terminal_unknown_typed_code_is_rejected() {
        let err =
            ensure_terminal_failure_code(ProviderFailureCode::Unknown).expect_err("must deny");
        assert_eq!(err, TerminalFailureCodeError::UnknownForbidden);
    }

    #[test]
    fn deserialization_rejects_unknown_typed_code() {
        let payload = serde_json::json!({
            "class": "unknown",
            "message": "bad",
            "typed_code": "unknown"
        });
        let err = serde_json::from_value::<HarnessFailure>(payload).expect_err("must reject");
        let msg = err.to_string();
        assert!(msg.contains("unknown is forbidden"), "{msg}");
    }

    proptest! {
        #[test]
        fn typed_codes_are_never_overridden_by_audit_fallback(noise in ".{0,120}") {
            let class = classify_provider_failure(&ProviderFailureContext {
                typed_code: ProviderFailureCode::Authentication,
                stdout_tail: Some(noise),
                stderr_tail: Some("429 too many requests temporary".into()),
                ..ProviderFailureContext::default()
            });
            prop_assert_eq!(class, HarnessFailureClass::Authentication);
        }

        #[test]
        fn audit_fallback_still_classifies_unknown_typed_code_from_exit_code(noise in ".{0,120}") {
            let class = classify_provider_failure_for_audit(&ProviderFailureContext {
                typed_code: ProviderFailureCode::Unknown,
                exit_code: Some(75),
                stderr_tail: Some(noise),
                ..ProviderFailureContext::default()
            });
            prop_assert_eq!(class, HarnessFailureClass::Transient);
        }
    }
}
