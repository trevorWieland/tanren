//! Shared telemetry primitives for the tanren workspace.
//!
//! # Responsibilities
//!
//! - Tracing subscriber initialization for binary crates
//! - Structured logging with configurable filter levels
//! - Foundation for future OpenTelemetry integration
//!
//! # Design Rules
//!
//! - No crate emits unstructured logs without correlation context
//! - All telemetry uses structured tracing, never `println!` or `eprintln!`
//! - Binary crates call [`init_tracing`] or [`init_tracing_for_contract_io`] once at startup

mod internal_error_sink;

use chrono::Utc;
use serde::Serialize;
use tanren_contract::{ErrorResponse, internal_error_response_with_correlation};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

pub use internal_error_sink::{INTERNAL_ERROR_SINK_PATH_ENV_VAR, spill_drop_count};

/// Errors that can occur during observability initialization.
#[derive(Debug, thiserror::Error)]
pub enum ObservabilityError {
    /// The log filter string could not be parsed.
    #[error("failed to parse log filter: {0}")]
    FilterParse(String),

    /// The tracing subscriber has already been initialized.
    #[error("tracing subscriber already initialized")]
    AlreadyInitialized,

    /// A correlated error event could not be serialized.
    #[error("failed to serialize correlated error event: {0}")]
    SinkSerialize(String),

    /// A correlated error event could not be written.
    #[error("failed to write correlated error event: {0}")]
    SinkIo(String),
}

/// Initialize the global tracing subscriber with the given filter level.
///
/// Call once at binary startup. Uses compact human-readable format
/// suitable for CLI and development use. Future lanes will add JSON
/// format for daemon/production use.
///
/// The `level` parameter accepts any valid [`EnvFilter`] directive:
/// - Simple levels: `"info"`, `"debug"`, `"warn"`
/// - Targeted: `"tanren=debug,sea_orm=warn"`
///
/// # Errors
///
/// Returns [`ObservabilityError::FilterParse`] if the level string is
/// invalid, or [`ObservabilityError::AlreadyInitialized`] if the global
/// subscriber was already set.
pub fn init_tracing(level: &str) -> Result<(), ObservabilityError> {
    let filter =
        EnvFilter::try_new(level).map_err(|e| ObservabilityError::FilterParse(e.to_string()))?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .try_init()
        .map_err(|_| ObservabilityError::AlreadyInitialized)
}

/// Initialize tracing for binaries with strict machine I/O contracts.
///
/// This variant validates and installs a global subscriber but writes
/// trace output to a sink so command stdout/stderr remain contract-only.
pub fn init_tracing_for_contract_io(level: &str) -> Result<(), ObservabilityError> {
    let filter =
        EnvFilter::try_new(level).map_err(|e| ObservabilityError::FilterParse(e.to_string()))?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::sink)
        .compact()
        .try_init()
        .map_err(|_| ObservabilityError::AlreadyInitialized)
}

/// Emit an internal error event to the default JSONL sink.
///
/// This writes through a durable sink path. Correlation IDs should only
/// be returned to callers when this function returns `Ok(())`.
pub fn emit_correlated_internal_error(
    component: &str,
    error_code: &str,
    correlation_id: Uuid,
    raw_error: &str,
) -> Result<(), ObservabilityError> {
    let paths = internal_error_sink::default_internal_error_sink_paths()?;
    let line = build_correlated_internal_error_jsonl_line(
        component,
        error_code,
        correlation_id,
        raw_error,
    )?;
    internal_error_sink::global_correlated_error_sink().emit(&paths, line)
}

/// Build a canonical internal-error `ErrorResponse` after emitting a
/// correlated entry to the default JSONL sink.
///
/// On sink-emission success, the response carries
/// [`ErrorDetails::Internal { correlation_id }`] so operators can trace
/// it back to its sink record. On sink failure, the response omits the
/// correlation id — we never surface an id the caller cannot look up.
///
/// [`ErrorDetails::Internal { correlation_id }`]: tanren_contract::ErrorDetails::Internal
pub fn emit_and_build_internal_error_response(
    component: &str,
    error_code: &str,
    raw_error: &str,
) -> ErrorResponse {
    emit_and_build_internal_error_response_with_emitter(
        component,
        error_code,
        raw_error,
        emit_correlated_internal_error,
    )
}

/// Testable variant of [`emit_and_build_internal_error_response`] that
/// accepts an injected emitter. Production callers should use the
/// non-`with_emitter` variant.
pub fn emit_and_build_internal_error_response_with_emitter(
    component: &str,
    error_code: &str,
    raw_error: &str,
    emitter: fn(&str, &str, Uuid, &str) -> Result<(), ObservabilityError>,
) -> ErrorResponse {
    internal_error_response_with_correlation::<ObservabilityError>(|correlation_id| {
        emitter(component, error_code, correlation_id, raw_error)
    })
}

/// Sanitize error text before structured logging.
///
/// This redacts URL userinfo segments (`scheme://user:pass@host`) and
/// common credential-like query parameters to reduce accidental secret
/// leakage in logs.
#[must_use]
pub fn sanitize_error_for_log(raw: &str) -> String {
    let redacted_url_userinfo = redact_url_userinfo(raw);
    redact_query_credentials(&redacted_url_userinfo)
}

fn redact_url_userinfo(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;

    while let Some(rel) = raw[i..].find("://") {
        let scheme_sep = i + rel;
        out.push_str(&raw[i..scheme_sep + 3]);
        let authority_start = scheme_sep + 3;
        let authority_end = raw[authority_start..]
            .find(['/', '?', '#', ' '])
            .map_or(raw.len(), |idx| authority_start + idx);
        let authority = &raw[authority_start..authority_end];

        if let Some(at_idx) = authority.rfind('@') {
            let host = &authority[at_idx + 1..];
            out.push_str("REDACTED@");
            out.push_str(host);
        } else {
            out.push_str(authority);
        }

        i = authority_end;
    }

    out.push_str(&raw[i..]);
    out
}

fn redact_query_credentials(raw: &str) -> String {
    let mut sanitized = raw.to_owned();
    for key in [
        "password", "passwd", "pwd", "token", "api_key", "apikey", "secret",
    ] {
        for prefix in [format!("{key}="), format!("{key}:"), format!("{key}%3d")] {
            sanitized = redact_after_prefix(&sanitized, &prefix);
            sanitized = redact_after_prefix(&sanitized, &prefix.to_ascii_uppercase());
        }
    }
    sanitized
}

fn redact_after_prefix(input: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut start = 0;

    while let Some(idx) = input[start..].find(prefix) {
        let absolute = start + idx;
        let value_start = absolute + prefix.len();
        out.push_str(&input[start..value_start]);
        let value_end = input[value_start..]
            .find(['&', ' ', ';', ',', '"', '\'', ')', ']', '}'])
            .map_or(input.len(), |end| value_start + end);
        out.push_str("REDACTED");
        start = value_end;
    }

    out.push_str(&input[start..]);
    out
}

fn build_correlated_internal_error_jsonl_line(
    component: &str,
    error_code: &str,
    correlation_id: Uuid,
    raw_error: &str,
) -> Result<String, ObservabilityError> {
    let record = CorrelatedInternalErrorRecord {
        timestamp_utc: Utc::now().to_rfc3339(),
        component,
        error_code,
        correlation_id: correlation_id.to_string(),
        message: sanitize_error_for_log(raw_error),
    };
    serde_json::to_string(&record).map_err(|err| ObservabilityError::SinkSerialize(err.to_string()))
}

#[derive(Debug, Serialize)]
struct CorrelatedInternalErrorRecord<'a> {
    timestamp_utc: String,
    component: &'a str,
    error_code: &'a str,
    correlation_id: String,
    message: String,
}
