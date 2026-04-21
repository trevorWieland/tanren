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
}

/// Normalized raw context adapters can pass into classification helpers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProviderFailureContext {
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

/// Best-effort classifier for provider-native failures.
#[must_use]
pub fn classify_provider_failure(ctx: &ProviderFailureContext) -> HarnessFailureClass {
    let mut merged = String::new();
    if let Some(value) = &ctx.provider_code {
        merged.push_str(value);
        merged.push('\n');
    }
    if let Some(value) = &ctx.provider_kind {
        merged.push_str(value);
        merged.push('\n');
    }
    if let Some(value) = &ctx.signal {
        merged.push_str(value);
        merged.push('\n');
    }
    if let Some(value) = &ctx.stdout_tail {
        merged.push_str(value);
        merged.push('\n');
    }
    if let Some(value) = &ctx.stderr_tail {
        merged.push_str(value);
    }
    let haystack = merged.to_ascii_lowercase();

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
    if contains_any(&haystack, &["invalid argument", "bad request", "malformed"]) {
        return HarnessFailureClass::InvalidRequest;
    }
    if ctx.exit_code == Some(137) {
        return HarnessFailureClass::ResourceExhausted;
    }
    if ctx.exit_code == Some(124) {
        return HarnessFailureClass::Timeout;
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
    use super::*;

    #[test]
    fn maps_rate_limit_to_transient() {
        let class = classify_provider_failure(&ProviderFailureContext {
            stderr_tail: Some("429 too many requests".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::RateLimited);
        assert_eq!(class.to_domain_error_class(), ErrorClass::Transient);
    }

    #[test]
    fn maps_capability_denial_to_fatal() {
        let class = classify_provider_failure(&ProviderFailureContext {
            provider_kind: Some("unsupported capability: tool_events".into()),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::CapabilityDenied);
        assert_eq!(class.to_domain_error_class(), ErrorClass::Fatal);
    }

    #[test]
    fn maps_exit_code_137_to_resource_exhausted() {
        let class = classify_provider_failure(&ProviderFailureContext {
            exit_code: Some(137),
            ..ProviderFailureContext::default()
        });
        assert_eq!(class, HarnessFailureClass::ResourceExhausted);
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
}
