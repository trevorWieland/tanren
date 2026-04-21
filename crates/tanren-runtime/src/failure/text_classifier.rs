use super::HarnessFailureClass;

const MAX_ANALYZED_CHANNEL_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum PhraseToken {
    #[default]
    Other,
    Unsupported,
    Capability,
    Approval,
    Denied,
    Consent,
    Invalid,
    Api,
    Key,
    Permission,
    Rate,
    Limit,
    Too,
    Many,
    Requests,
    Deadline,
    Exceeded,
    Timed,
    Out,
    Connection,
    Refused,
    Network,
    Unreachable,
    Temporarily,
    Unavailable,
    Resource,
    Exhausted,
    Quota,
    Exit,
    Code,
    Try,
    Again,
    Bad,
    Request,
    Internal,
    Error,
    Argument,
    Of,
    Memory,
    Num137,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum Feature {
    CapabilityDeniedToken,
    UnsupportedCapabilityPhrase,
    ApprovalDeniedPhrase,
    ApprovalRequiredToken,
    ConsentDeniedPhrase,
    AuthenticationToken,
    InvalidApiKeyToken,
    InvalidApiKeyPhrase,
    PermissionDeniedPhrase,
    Has401,
    Has403,
    RateLimitPhrase,
    RateLimitedToken,
    TooManyRequestsToken,
    TooManyRequestsPhrase,
    Has429,
    TimeoutToken,
    DeadlineExceededToken,
    TimedOutToken,
    DeadlineExceededPhrase,
    TimedOutPhrase,
    ConnectionRefusedPhrase,
    NetworkUnreachablePhrase,
    DnsToken,
    EconnresetToken,
    TemporarilyUnavailablePhrase,
    Has503,
    OutOfMemoryPhrase,
    ResourceExhaustedPhrase,
    QuotaExceededPhrase,
    ExitCode137Phrase,
    TemporaryToken,
    RetryableToken,
    TransientToken,
    EagainToken,
    TryAgainPhrase,
    InvalidArgumentPhrase,
    BadRequestPhrase,
    MalformedToken,
    PanicToken,
    FatalToken,
    InternalErrorToken,
    InternalErrorPhrase,
}

#[derive(Debug, Default, Clone, Copy)]
struct TokenFeatures(u128);

impl TokenFeatures {
    fn set(&mut self, feature: Feature) {
        self.0 |= 1_u128 << (feature as u8);
    }

    fn has(self, feature: Feature) -> bool {
        self.0 & (1_u128 << (feature as u8)) != 0
    }
}

const FEATURE_TOKEN_MAP: [(&[u8], Feature); 17] = [
    (b"capability_denied", Feature::CapabilityDeniedToken),
    (b"approval_required", Feature::ApprovalRequiredToken),
    (b"authentication", Feature::AuthenticationToken),
    (b"invalid_api_key", Feature::InvalidApiKeyToken),
    (b"401", Feature::Has401),
    (b"403", Feature::Has403),
    (b"rate_limited", Feature::RateLimitedToken),
    (b"too_many_requests", Feature::TooManyRequestsToken),
    (b"429", Feature::Has429),
    (b"timeout", Feature::TimeoutToken),
    (b"deadline_exceeded", Feature::DeadlineExceededToken),
    (b"timed_out", Feature::TimedOutToken),
    (b"dns", Feature::DnsToken),
    (b"econnreset", Feature::EconnresetToken),
    (b"503", Feature::Has503),
    (b"malformed", Feature::MalformedToken),
    (b"internal_error", Feature::InternalErrorToken),
];

const FEATURE_TOKEN_MAP_2: [(&[u8], Feature); 4] = [
    (b"temporary", Feature::TemporaryToken),
    (b"retryable", Feature::RetryableToken),
    (b"transient", Feature::TransientToken),
    (b"eagain", Feature::EagainToken),
];

const FEATURE_TOKEN_MAP_3: [(&[u8], Feature); 2] = [
    (b"panic", Feature::PanicToken),
    (b"fatal", Feature::FatalToken),
];

const PHRASE_TOKEN_MAP: [(&[u8], PhraseToken); 39] = [
    (b"unsupported", PhraseToken::Unsupported),
    (b"capability", PhraseToken::Capability),
    (b"approval", PhraseToken::Approval),
    (b"denied", PhraseToken::Denied),
    (b"consent", PhraseToken::Consent),
    (b"invalid", PhraseToken::Invalid),
    (b"api", PhraseToken::Api),
    (b"key", PhraseToken::Key),
    (b"permission", PhraseToken::Permission),
    (b"rate", PhraseToken::Rate),
    (b"limit", PhraseToken::Limit),
    (b"too", PhraseToken::Too),
    (b"many", PhraseToken::Many),
    (b"requests", PhraseToken::Requests),
    (b"deadline", PhraseToken::Deadline),
    (b"exceeded", PhraseToken::Exceeded),
    (b"timed", PhraseToken::Timed),
    (b"out", PhraseToken::Out),
    (b"connection", PhraseToken::Connection),
    (b"refused", PhraseToken::Refused),
    (b"network", PhraseToken::Network),
    (b"unreachable", PhraseToken::Unreachable),
    (b"temporarily", PhraseToken::Temporarily),
    (b"unavailable", PhraseToken::Unavailable),
    (b"resource", PhraseToken::Resource),
    (b"exhausted", PhraseToken::Exhausted),
    (b"quota", PhraseToken::Quota),
    (b"exit", PhraseToken::Exit),
    (b"code", PhraseToken::Code),
    (b"try", PhraseToken::Try),
    (b"again", PhraseToken::Again),
    (b"bad", PhraseToken::Bad),
    (b"request", PhraseToken::Request),
    (b"internal", PhraseToken::Internal),
    (b"error", PhraseToken::Error),
    (b"argument", PhraseToken::Argument),
    (b"of", PhraseToken::Of),
    (b"memory", PhraseToken::Memory),
    (b"137", PhraseToken::Num137),
];

const PHRASE_MAP_2: [((PhraseToken, PhraseToken), Feature); 16] = [
    (
        (PhraseToken::Unsupported, PhraseToken::Capability),
        Feature::UnsupportedCapabilityPhrase,
    ),
    (
        (PhraseToken::Approval, PhraseToken::Denied),
        Feature::ApprovalDeniedPhrase,
    ),
    (
        (PhraseToken::Consent, PhraseToken::Denied),
        Feature::ConsentDeniedPhrase,
    ),
    (
        (PhraseToken::Permission, PhraseToken::Denied),
        Feature::PermissionDeniedPhrase,
    ),
    (
        (PhraseToken::Rate, PhraseToken::Limit),
        Feature::RateLimitPhrase,
    ),
    (
        (PhraseToken::Deadline, PhraseToken::Exceeded),
        Feature::DeadlineExceededPhrase,
    ),
    (
        (PhraseToken::Timed, PhraseToken::Out),
        Feature::TimedOutPhrase,
    ),
    (
        (PhraseToken::Connection, PhraseToken::Refused),
        Feature::ConnectionRefusedPhrase,
    ),
    (
        (PhraseToken::Network, PhraseToken::Unreachable),
        Feature::NetworkUnreachablePhrase,
    ),
    (
        (PhraseToken::Temporarily, PhraseToken::Unavailable),
        Feature::TemporarilyUnavailablePhrase,
    ),
    (
        (PhraseToken::Resource, PhraseToken::Exhausted),
        Feature::ResourceExhaustedPhrase,
    ),
    (
        (PhraseToken::Quota, PhraseToken::Exceeded),
        Feature::QuotaExceededPhrase,
    ),
    (
        (PhraseToken::Try, PhraseToken::Again),
        Feature::TryAgainPhrase,
    ),
    (
        (PhraseToken::Bad, PhraseToken::Request),
        Feature::BadRequestPhrase,
    ),
    (
        (PhraseToken::Invalid, PhraseToken::Argument),
        Feature::InvalidArgumentPhrase,
    ),
    (
        (PhraseToken::Internal, PhraseToken::Error),
        Feature::InternalErrorPhrase,
    ),
];

const PHRASE_MAP_3: [((PhraseToken, PhraseToken, PhraseToken), Feature); 4] = [
    (
        (PhraseToken::Invalid, PhraseToken::Api, PhraseToken::Key),
        Feature::InvalidApiKeyPhrase,
    ),
    (
        (PhraseToken::Too, PhraseToken::Many, PhraseToken::Requests),
        Feature::TooManyRequestsPhrase,
    ),
    (
        (PhraseToken::Out, PhraseToken::Of, PhraseToken::Memory),
        Feature::OutOfMemoryPhrase,
    ),
    (
        (PhraseToken::Exit, PhraseToken::Code, PhraseToken::Num137),
        Feature::ExitCode137Phrase,
    ),
];

pub(super) fn classify_text_fallback(
    stdout_tail: Option<&str>,
    stderr_tail: Option<&str>,
) -> HarnessFailureClass {
    let mut features = TokenFeatures::default();
    if let Some(stdout) = stdout_tail {
        scan_tokens(tail_slice(stdout), &mut features);
    }
    if let Some(stderr) = stderr_tail {
        scan_tokens(tail_slice(stderr), &mut features);
    }

    if features.has(Feature::CapabilityDeniedToken)
        || features.has(Feature::UnsupportedCapabilityPhrase)
    {
        return HarnessFailureClass::CapabilityDenied;
    }
    if features.has(Feature::ApprovalDeniedPhrase)
        || features.has(Feature::ApprovalRequiredToken)
        || features.has(Feature::ConsentDeniedPhrase)
    {
        return HarnessFailureClass::ApprovalDenied;
    }
    if features.has(Feature::AuthenticationToken)
        || features.has(Feature::InvalidApiKeyToken)
        || features.has(Feature::InvalidApiKeyPhrase)
        || features.has(Feature::PermissionDeniedPhrase)
        || features.has(Feature::Has401)
        || features.has(Feature::Has403)
    {
        return HarnessFailureClass::Authentication;
    }
    if features.has(Feature::RateLimitPhrase)
        || features.has(Feature::RateLimitedToken)
        || features.has(Feature::TooManyRequestsToken)
        || features.has(Feature::TooManyRequestsPhrase)
        || features.has(Feature::Has429)
    {
        return HarnessFailureClass::RateLimited;
    }
    if features.has(Feature::TimeoutToken)
        || features.has(Feature::DeadlineExceededToken)
        || features.has(Feature::TimedOutToken)
        || features.has(Feature::DeadlineExceededPhrase)
        || features.has(Feature::TimedOutPhrase)
    {
        return HarnessFailureClass::Timeout;
    }
    if features.has(Feature::ConnectionRefusedPhrase)
        || features.has(Feature::NetworkUnreachablePhrase)
        || features.has(Feature::DnsToken)
        || features.has(Feature::EconnresetToken)
        || features.has(Feature::TemporarilyUnavailablePhrase)
        || features.has(Feature::Has503)
    {
        return HarnessFailureClass::TransportUnavailable;
    }
    if features.has(Feature::OutOfMemoryPhrase)
        || features.has(Feature::ResourceExhaustedPhrase)
        || features.has(Feature::QuotaExceededPhrase)
        || features.has(Feature::ExitCode137Phrase)
    {
        return HarnessFailureClass::ResourceExhausted;
    }
    if features.has(Feature::TemporaryToken)
        || features.has(Feature::RetryableToken)
        || features.has(Feature::TransientToken)
        || features.has(Feature::EagainToken)
        || features.has(Feature::TryAgainPhrase)
    {
        return HarnessFailureClass::Transient;
    }
    if features.has(Feature::InvalidArgumentPhrase)
        || features.has(Feature::BadRequestPhrase)
        || features.has(Feature::MalformedToken)
    {
        return HarnessFailureClass::InvalidRequest;
    }
    if features.has(Feature::PanicToken)
        || features.has(Feature::FatalToken)
        || features.has(Feature::InternalErrorToken)
        || features.has(Feature::InternalErrorPhrase)
    {
        return HarnessFailureClass::Fatal;
    }

    HarnessFailureClass::Unknown
}

fn tail_slice(value: &str) -> &str {
    if value.len() <= MAX_ANALYZED_CHANNEL_BYTES {
        return value;
    }

    let mut start = value.len() - MAX_ANALYZED_CHANNEL_BYTES;
    while start < value.len() && !value.is_char_boundary(start) {
        start += 1;
    }
    &value[start..]
}

fn scan_tokens(text: &str, features: &mut TokenFeatures) {
    let bytes = text.as_bytes();
    let mut cursor = 0;
    let mut prev2 = PhraseToken::Other;
    let mut prev1 = PhraseToken::Other;

    while cursor < bytes.len() {
        while cursor < bytes.len() && !is_token_char(bytes[cursor]) {
            cursor += 1;
        }
        let start = cursor;
        while cursor < bytes.len() && is_token_char(bytes[cursor]) {
            cursor += 1;
        }
        if start == cursor {
            continue;
        }

        let token = &bytes[start..cursor];
        set_single_token_features(token, features);

        let current = phrase_token(token);
        set_phrase_features(prev2, prev1, current, features);
        prev2 = prev1;
        prev1 = current;
    }
}

const fn is_token_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn set_single_token_features(token: &[u8], features: &mut TokenFeatures) {
    for (candidate, feature) in FEATURE_TOKEN_MAP {
        if token_eq(token, candidate) {
            features.set(feature);
        }
    }
    for (candidate, feature) in FEATURE_TOKEN_MAP_2 {
        if token_eq(token, candidate) {
            features.set(feature);
        }
    }
    for (candidate, feature) in FEATURE_TOKEN_MAP_3 {
        if token_eq(token, candidate) {
            features.set(feature);
        }
    }
}

fn set_phrase_features(
    prev2: PhraseToken,
    prev1: PhraseToken,
    current: PhraseToken,
    features: &mut TokenFeatures,
) {
    for ((left, right), feature) in PHRASE_MAP_2 {
        if prev1 == left && current == right {
            features.set(feature);
        }
    }

    for ((left, mid, right), feature) in PHRASE_MAP_3 {
        if prev2 == left && current == right && (mid == PhraseToken::Other || prev1 == mid) {
            features.set(feature);
        }
    }
}

fn token_eq(token: &[u8], expected: &[u8]) -> bool {
    token.len() == expected.len()
        && token
            .iter()
            .zip(expected.iter())
            .all(|(actual, wanted)| actual.eq_ignore_ascii_case(wanted))
}

fn phrase_token(token: &[u8]) -> PhraseToken {
    for (candidate, phrase) in PHRASE_TOKEN_MAP {
        if token_eq(token, candidate) {
            return phrase;
        }
    }
    PhraseToken::Other
}
