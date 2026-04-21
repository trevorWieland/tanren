use serde::{Deserialize, Serialize};
use tanren_domain::ErrorClass;

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

/// Typed failure payload returned by harness adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{class:?}: {message}")]
pub struct HarnessFailure {
    pub class: HarnessFailureClass,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_code: Option<ProviderFailureCode>,
}

/// Normalized raw context adapters can pass into classification helpers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProviderFailureContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_code: Option<ProviderFailureCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_tail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_tail: Option<String>,
}

/// Classify provider-native failures into stable harness classes.
///
/// Order of precedence:
/// 1) typed code from adapter
/// 2) deterministic exact-token mappings from structured fields
/// 3) deterministic exit/signal mappings
/// 4) free-text fallback heuristics
#[must_use]
pub fn classify_provider_failure(ctx: &ProviderFailureContext) -> HarnessFailureClass {
    if let Some(code) = ctx.typed_code {
        return code.to_harness_failure_class();
    }

    if let Some(class) = ctx
        .provider_code
        .as_deref()
        .and_then(classify_exact_identifier)
    {
        return class;
    }

    if let Some(class) = ctx
        .provider_kind
        .as_deref()
        .and_then(classify_exact_identifier)
    {
        return class;
    }

    if let Some(class) = ctx.signal.as_deref().and_then(classify_exact_identifier) {
        return class;
    }

    if let Some(class) = ctx.exit_code.and_then(classify_exit_code) {
        return class;
    }

    let mut merged = String::new();
    if let Some(value) = &ctx.stdout_tail {
        merged.push_str(value);
        merged.push('\n');
    }
    if let Some(value) = &ctx.stderr_tail {
        merged.push_str(value);
    }

    classify_text_fallback(&merged)
}

fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn classify_exact_identifier(raw: &str) -> Option<HarnessFailureClass> {
    let token = normalize_identifier(raw);
    match token.as_str() {
        "capability_denied" | "unsupported_capability" => {
            Some(HarnessFailureClass::CapabilityDenied)
        }
        "approval_denied" | "approval_required" | "consent_denied" => {
            Some(HarnessFailureClass::ApprovalDenied)
        }
        "authentication" | "unauthorized" | "forbidden" | "invalid_api_key" | "401" | "403" => {
            Some(HarnessFailureClass::Authentication)
        }
        "rate_limited" | "rate_limit" | "too_many_requests" | "429" => {
            Some(HarnessFailureClass::RateLimited)
        }
        "timeout" | "deadline_exceeded" | "timed_out" | "124" => Some(HarnessFailureClass::Timeout),
        "transport_unavailable"
        | "connection_refused"
        | "network_unreachable"
        | "dns"
        | "econnreset"
        | "503" => Some(HarnessFailureClass::TransportUnavailable),
        "resource_exhausted" | "out_of_memory" | "quota_exceeded" | "137" => {
            Some(HarnessFailureClass::ResourceExhausted)
        }
        "invalid_request" | "invalid_argument" | "bad_request" | "malformed" | "400" => {
            Some(HarnessFailureClass::InvalidRequest)
        }
        "transient" | "temporary" | "temporarily_unavailable" | "retryable" | "eagain" | "75" => {
            Some(HarnessFailureClass::Transient)
        }
        "fatal" | "panic" | "internal_error" => Some(HarnessFailureClass::Fatal),
        "unknown" => Some(HarnessFailureClass::Unknown),
        _ => None,
    }
}

const fn classify_exit_code(code: i32) -> Option<HarnessFailureClass> {
    match code {
        124 => Some(HarnessFailureClass::Timeout),
        137 => Some(HarnessFailureClass::ResourceExhausted),
        75 => Some(HarnessFailureClass::Transient),
        _ => None,
    }
}

fn classify_text_fallback(text: &str) -> HarnessFailureClass {
    let haystack = text.to_ascii_lowercase();

    if contains_any(&haystack, &["capability_denied", "unsupported capability"]) {
        return HarnessFailureClass::CapabilityDenied;
    }
    if contains_any(
        &haystack,
        &["approval denied", "approval_required", "consent denied"],
    ) {
        return HarnessFailureClass::ApprovalDenied;
    }
    if contains_any(
        &haystack,
        &[
            "authentication",
            "invalid api key",
            "permission denied",
            "401",
            "403",
        ],
    ) {
        return HarnessFailureClass::Authentication;
    }
    if contains_any(
        &haystack,
        &["rate limit", "rate_limited", "too many requests", "429"],
    ) {
        return HarnessFailureClass::RateLimited;
    }
    if contains_any(&haystack, &["timeout", "deadline exceeded", "timed out"]) {
        return HarnessFailureClass::Timeout;
    }
    if contains_any(
        &haystack,
        &[
            "connection refused",
            "network unreachable",
            "dns",
            "econnreset",
            "temporarily unavailable",
            "503",
        ],
    ) {
        return HarnessFailureClass::TransportUnavailable;
    }
    if contains_any(
        &haystack,
        &[
            "out of memory",
            "resource exhausted",
            "quota exceeded",
            "exit code 137",
        ],
    ) {
        return HarnessFailureClass::ResourceExhausted;
    }
    if contains_any(
        &haystack,
        &["temporary", "retryable", "transient", "try again", "eagain"],
    ) {
        return HarnessFailureClass::Transient;
    }
    if contains_any(&haystack, &["invalid argument", "bad request", "malformed"]) {
        return HarnessFailureClass::InvalidRequest;
    }
    if contains_any(&haystack, &["panic", "fatal", "internal error"]) {
        return HarnessFailureClass::Fatal;
    }

    HarnessFailureClass::Unknown
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
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
    fn maps_exact_provider_code_without_text_fallback() {
        let class = classify_provider_failure(&ProviderFailureContext {
            provider_code: Some("rate_limited".into()),
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
    }

    #[test]
    fn maps_rate_limit_to_transient_domain_error_class() {
        let class = classify_provider_failure(&ProviderFailureContext {
            provider_code: Some("429".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::RateLimited);
        assert_eq!(class.to_domain_error_class(), ErrorClass::Transient);
    }

    #[test]
    fn maps_capability_denial_to_fatal_domain_error_class() {
        let class = classify_provider_failure(&ProviderFailureContext {
            provider_kind: Some("unsupported_capability".into()),
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
