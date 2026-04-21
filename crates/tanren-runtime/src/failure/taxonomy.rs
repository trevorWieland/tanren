use super::HarnessFailureClass;

pub(super) struct FailureTaxonomyRule {
    pub(super) class: HarnessFailureClass,
    pub(super) exact_identifiers: &'static [&'static str],
    pub(super) text_tokens: &'static [&'static str],
    pub(super) text_phrases: &'static [&'static [&'static str]],
}

const CAPABILITY_PHRASES: &[&[&str]] = &[&["unsupported", "capability"], &["not", "supported"]];
const APPROVAL_PHRASES: &[&[&str]] = &[
    &["approval", "denied"],
    &["consent", "denied"],
    &["user", "rejected"],
];
const AUTH_PHRASES: &[&[&str]] = &[
    &["invalid", "api", "key"],
    &["permission", "denied"],
    &["access", "denied"],
];
const RATE_LIMIT_PHRASES: &[&[&str]] = &[
    &["rate", "limit"],
    &["rate", "limited"],
    &["too", "many", "requests"],
];
const TIMEOUT_PHRASES: &[&[&str]] = &[
    &["deadline", "exceeded"],
    &["timed", "out"],
    &["request", "timed", "out"],
    &["gateway", "timeout"],
];
const TRANSPORT_PHRASES: &[&[&str]] = &[
    &["connection", "refused"],
    &["network", "unreachable"],
    &["service", "unavailable"],
    &["connection", "timed", "out"],
];
const RESOURCE_PHRASES: &[&[&str]] = &[
    &["out", "of", "memory"],
    &["resource", "exhausted"],
    &["quota", "exceeded"],
    &["context", "length", "exceeded"],
    &["max", "tokens", "exceeded"],
    &["exit", "code", "137"],
];
const INVALID_REQUEST_PHRASES: &[&[&str]] = &[
    &["invalid", "argument"],
    &["bad", "request"],
    &["unprocessable", "entity"],
];
const TRANSIENT_PHRASES: &[&[&str]] = &[
    &["try", "again"],
    &["please", "retry"],
    &["temporarily", "unavailable"],
];
const FATAL_PHRASES: &[&[&str]] = &[&["internal", "error"], &["fatal", "error"]];

pub(super) const FAILURE_TAXONOMY: &[FailureTaxonomyRule] = &[
    FailureTaxonomyRule {
        class: HarnessFailureClass::CapabilityDenied,
        exact_identifiers: &[
            "capability_denied",
            "unsupported_capability",
            "unsupported_feature",
            "not_supported",
        ],
        text_tokens: &["capability_denied", "unsupported_capability"],
        text_phrases: CAPABILITY_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::ApprovalDenied,
        exact_identifiers: &[
            "approval_denied",
            "approval_required",
            "consent_denied",
            "user_rejected",
            "requires_approval",
        ],
        text_tokens: &["approval_denied", "approval_required", "consent_denied"],
        text_phrases: APPROVAL_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::Authentication,
        exact_identifiers: &[
            "authentication",
            "auth_error",
            "auth_failed",
            "unauthorized",
            "unauthenticated",
            "forbidden",
            "invalid_api_key",
            "invalid_token",
            "expired_token",
            "permission_denied",
            "access_denied",
            "401",
            "403",
        ],
        text_tokens: &[
            "authentication",
            "auth_error",
            "auth_failed",
            "unauthorized",
            "unauthenticated",
            "forbidden",
            "invalid_api_key",
            "invalid_token",
            "expired_token",
            "permission_denied",
            "access_denied",
            "401",
            "403",
        ],
        text_phrases: AUTH_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::RateLimited,
        exact_identifiers: &[
            "rate_limited",
            "rate_limit",
            "too_many_requests",
            "throttled",
            "throttling",
            "quota_rate_limited",
            "429",
        ],
        text_tokens: &[
            "rate_limited",
            "rate_limit",
            "too_many_requests",
            "throttled",
            "throttling",
            "429",
        ],
        text_phrases: RATE_LIMIT_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::Timeout,
        exact_identifiers: &[
            "timeout",
            "deadline_exceeded",
            "timed_out",
            "request_timeout",
            "gateway_timeout",
            "operation_timed_out",
            "124",
            "408",
            "504",
        ],
        text_tokens: &[
            "timeout",
            "deadline_exceeded",
            "timed_out",
            "request_timeout",
            "gateway_timeout",
            "operation_timed_out",
            "124",
            "408",
            "504",
        ],
        text_phrases: TIMEOUT_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::TransportUnavailable,
        exact_identifiers: &[
            "transport_unavailable",
            "connection_refused",
            "network_unreachable",
            "dns",
            "econnreset",
            "econnrefused",
            "enotfound",
            "service_unavailable",
            "connection_timeout",
            "502",
            "503",
        ],
        text_tokens: &[
            "transport_unavailable",
            "connection_refused",
            "network_unreachable",
            "dns",
            "econnreset",
            "econnrefused",
            "enotfound",
            "service_unavailable",
            "connection_timeout",
            "502",
            "503",
        ],
        text_phrases: TRANSPORT_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::ResourceExhausted,
        exact_identifiers: &[
            "resource_exhausted",
            "out_of_memory",
            "oom",
            "quota_exceeded",
            "context_length_exceeded",
            "max_tokens_exceeded",
            "137",
        ],
        text_tokens: &[
            "resource_exhausted",
            "out_of_memory",
            "oom",
            "quota_exceeded",
            "context_length_exceeded",
            "max_tokens_exceeded",
            "137",
        ],
        text_phrases: RESOURCE_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::Transient,
        exact_identifiers: &[
            "transient",
            "temporary",
            "temporarily_unavailable",
            "retryable",
            "eagain",
            "try_again",
            "backoff_required",
            "sigterm",
            "interrupted",
            "75",
        ],
        text_tokens: &[
            "transient",
            "temporary",
            "temporarily_unavailable",
            "retryable",
            "eagain",
            "try_again",
            "backoff_required",
            "sigterm",
            "interrupted",
            "75",
        ],
        text_phrases: TRANSIENT_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::InvalidRequest,
        exact_identifiers: &[
            "invalid_request",
            "invalid_argument",
            "bad_request",
            "malformed",
            "unprocessable_entity",
            "invalid_payload",
            "400",
            "422",
        ],
        text_tokens: &[
            "invalid_request",
            "invalid_argument",
            "bad_request",
            "malformed",
            "unprocessable_entity",
            "invalid_payload",
            "400",
            "422",
        ],
        text_phrases: INVALID_REQUEST_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::Fatal,
        exact_identifiers: &[
            "fatal",
            "panic",
            "internal_error",
            "assertion_failed",
            "unrecoverable",
        ],
        text_tokens: &[
            "fatal",
            "panic",
            "internal_error",
            "assertion_failed",
            "unrecoverable",
        ],
        text_phrases: FATAL_PHRASES,
    },
    FailureTaxonomyRule {
        class: HarnessFailureClass::Unknown,
        exact_identifiers: &["unknown"],
        text_tokens: &[],
        text_phrases: &[],
    },
];

pub(super) fn normalize_identifier(raw: &str) -> String {
    raw.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

pub(super) fn classify_exact_identifier(raw: &str) -> Option<HarnessFailureClass> {
    let token = normalize_identifier(raw);
    FAILURE_TAXONOMY.iter().find_map(|rule| {
        rule.exact_identifiers
            .iter()
            .any(|candidate| *candidate == token)
            .then_some(rule.class)
    })
}

pub(super) const fn classify_exit_code(code: i32) -> Option<HarnessFailureClass> {
    match code {
        124 => Some(HarnessFailureClass::Timeout),
        137 => Some(HarnessFailureClass::ResourceExhausted),
        143 | 75 => Some(HarnessFailureClass::Transient),
        _ => None,
    }
}
