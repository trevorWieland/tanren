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
    Unknown,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_code: Option<ProviderFailureCode>,
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
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            context: ProviderFailureContext::default(),
        }
    }

    #[must_use]
    pub fn with_context(mut self, context: ProviderFailureContext) -> Self {
        self.context = context;
        self
    }

    #[must_use]
    pub fn into_harness_failure(self) -> HarnessFailure {
        HarnessFailure::from_provider_failure(self)
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    typed_code: Option<ProviderFailureCode>,
}

impl HarnessFailure {
    #[must_use]
    pub fn new(class: HarnessFailureClass, message: impl Into<String>) -> Self {
        Self {
            class,
            message: message.into(),
            provider_code: None,
            provider_kind: None,
            typed_code: None,
        }
    }

    #[must_use]
    pub fn from_provider_failure(failure: ProviderFailure) -> Self {
        let class = classify_provider_failure(&failure.context);
        Self::from_provider_failure_with_class(failure, class)
    }

    #[must_use]
    pub fn from_provider_failure_with_class(
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
    pub const fn typed_code(&self) -> Option<ProviderFailureCode> {
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
    typed_code: Option<ProviderFailureCode>,
}

impl<'de> Deserialize<'de> for HarnessFailure {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = HarnessFailureWire::deserialize(deserializer)?;
        if let Some(typed_code) = wire.typed_code {
            let expected = typed_code.to_harness_failure_class();
            if wire.class != expected {
                return Err(serde::de::Error::custom(format!(
                    "typed_code {typed_code:?} implies class {expected:?}, got {:?}",
                    wire.class
                )));
            }
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
/// Order of precedence:
/// 1) typed code from adapter
/// 2) deterministic exact-token mappings from structured fields
/// 3) deterministic exit/signal mappings
/// 4) bounded boundary-aware text fallback heuristics
#[must_use]
pub fn classify_provider_failure(ctx: &ProviderFailureContext) -> HarnessFailureClass {
    if let Some(code) = ctx.typed_code {
        return code.to_harness_failure_class();
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
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn typed_code_has_priority_over_text_heuristics() {
        let class = classify_provider_failure(&ProviderFailureContext {
            typed_code: Some(ProviderFailureCode::Timeout),
            stderr_tail: Some("401 invalid api key".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::Timeout);
    }

    #[test]
    fn provider_failure_normalizes_through_context_classification() {
        let provider_failure =
            ProviderFailure::new("raw adapter failure").with_context(ProviderFailureContext {
                typed_code: Some(ProviderFailureCode::RateLimited),
                stderr_tail: Some("fatal panic".into()),
                ..ProviderFailureContext::default()
            });

        let failure = provider_failure.into_harness_failure();
        assert_eq!(failure.class(), HarnessFailureClass::RateLimited);
        assert_eq!(failure.typed_code(), Some(ProviderFailureCode::RateLimited));
    }

    #[test]
    fn maps_exact_provider_code_without_text_fallback() {
        let class = classify_provider_failure(&ProviderFailureContext {
            provider_code: Some(ProviderIdentifier::try_new("rate_limited").expect("code")),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::RateLimited);
    }

    #[test]
    fn maps_transient_from_structured_signal_or_exit_code() {
        let from_signal = classify_provider_failure(&ProviderFailureContext {
            signal: Some("temporary".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(from_signal, HarnessFailureClass::Transient);

        let from_exit = classify_provider_failure(&ProviderFailureContext {
            exit_code: Some(75),
            ..ProviderFailureContext::default()
        });
        assert_eq!(from_exit, HarnessFailureClass::Transient);

        let from_sigterm_exit = classify_provider_failure(&ProviderFailureContext {
            exit_code: Some(143),
            ..ProviderFailureContext::default()
        });
        assert_eq!(from_sigterm_exit, HarnessFailureClass::Transient);
    }

    #[test]
    fn maps_rate_limit_to_transient_domain_error_class() {
        let class = classify_provider_failure(&ProviderFailureContext {
            provider_code: Some(ProviderIdentifier::try_new("429").expect("code")),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::RateLimited);
        assert_eq!(class.to_domain_error_class(), ErrorClass::Transient);
    }

    #[test]
    fn maps_capability_denial_to_fatal_domain_error_class() {
        let class = classify_provider_failure(&ProviderFailureContext {
            provider_kind: Some(
                ProviderIdentifier::try_new("unsupported_capability").expect("kind"),
            ),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::CapabilityDenied);
        assert_eq!(class.to_domain_error_class(), ErrorClass::Fatal);
    }

    #[test]
    fn fallback_heuristics_only_used_when_structured_data_missing() {
        let class = classify_provider_failure(&ProviderFailureContext {
            stderr_tail: Some("429 too many requests".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::RateLimited);
    }

    #[test]
    fn classification_covers_common_provider_variants() {
        let cases = [
            ("auth_failed", HarnessFailureClass::Authentication),
            ("throttling", HarnessFailureClass::RateLimited),
            ("gateway_timeout", HarnessFailureClass::Timeout),
            ("econnrefused", HarnessFailureClass::TransportUnavailable),
            (
                "context_length_exceeded",
                HarnessFailureClass::ResourceExhausted,
            ),
            ("unprocessable_entity", HarnessFailureClass::InvalidRequest),
            ("backoff_required", HarnessFailureClass::Transient),
            ("assertion_failed", HarnessFailureClass::Fatal),
        ];
        for (provider_code, expected) in cases {
            let class = classify_provider_failure(&ProviderFailureContext {
                provider_code: Some(ProviderIdentifier::try_new(provider_code).expect("code")),
                ..ProviderFailureContext::default()
            });
            assert_eq!(class, expected, "provider_code={provider_code}");
        }
    }

    #[test]
    fn fallback_ignores_numeric_substrings_without_token_boundaries() {
        let class = classify_provider_failure(&ProviderFailureContext {
            stderr_tail: Some("artifact_1401 metrics_2403".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::Unknown);
    }

    #[test]
    fn fallback_scans_bounded_tail_for_large_output() {
        let class = classify_provider_failure(&ProviderFailureContext {
            stderr_tail: Some(format!("{} timeout", "x".repeat(24 * 1024))),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::Timeout);
    }

    #[test]
    fn fallback_phrase_detection_uses_shared_taxonomy() {
        let class = classify_provider_failure(&ProviderFailureContext {
            stderr_tail: Some("provider returned permission denied for token".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::Authentication);

        let class = classify_provider_failure(&ProviderFailureContext {
            stderr_tail: Some("service unavailable while dialing upstream".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::TransportUnavailable);
    }

    #[test]
    fn maps_unknown_to_ambiguous() {
        let class = classify_provider_failure(&ProviderFailureContext {
            stderr_tail: Some("something odd happened".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::Unknown);
        assert_eq!(class.to_domain_error_class(), ErrorClass::Ambiguous);
    }

    proptest! {
        #[test]
        fn typed_codes_are_never_overridden_by_fallback_text(noise in ".{0,120}") {
            let class = classify_provider_failure(&ProviderFailureContext {
                typed_code: Some(ProviderFailureCode::Authentication),
                stdout_tail: Some(noise),
                stderr_tail: Some("429 too many requests temporary".into()),
                ..ProviderFailureContext::default()
            });
            prop_assert_eq!(class, HarnessFailureClass::Authentication);
        }

        #[test]
        fn exit_code_75_remains_transient_even_with_unrelated_text(noise in ".{0,120}") {
            let class = classify_provider_failure(&ProviderFailureContext {
                exit_code: Some(75),
                stderr_tail: Some(noise),
                ..ProviderFailureContext::default()
            });
            prop_assert_eq!(class, HarnessFailureClass::Transient);
        }
    }
}
