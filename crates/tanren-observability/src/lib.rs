//! Shared telemetry primitives for the tanren workspace.
//!
//! # Responsibilities
//!
//! - Tracing subscriber initialization for binary crates
//! - Structured logging with configurable filter levels
//! - Foundation for future OpenTelemetry integration (Lane 0.5+)
//!
//! # Design Rules
//!
//! - No crate emits unstructured logs without correlation context
//! - All telemetry uses structured tracing, never `println!` or `eprintln!`
//! - Binary crates call [`init_tracing`] once at startup

use tracing_subscriber::EnvFilter;

/// Errors that can occur during observability initialization.
#[derive(Debug, thiserror::Error)]
pub enum ObservabilityError {
    /// The log filter string could not be parsed.
    #[error("failed to parse log filter: {0}")]
    FilterParse(String),

    /// The tracing subscriber has already been initialized.
    #[error("tracing subscriber already initialized")]
    AlreadyInitialized,
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

#[cfg(test)]
mod tests {
    use super::sanitize_error_for_log;

    #[test]
    fn sanitize_redacts_postgres_url_userinfo() {
        let raw = "failed to connect postgres://alice:supersecret@localhost:5432/tanren";
        let sanitized = sanitize_error_for_log(raw);
        assert!(!sanitized.contains("alice:supersecret"));
        assert!(sanitized.contains("postgres://REDACTED@localhost:5432/tanren"));
    }

    #[test]
    fn sanitize_redacts_sqlite_url_userinfo_and_query_secret() {
        let raw = "bad url sqlite://user:pass@localhost/tmp/t.db?mode=rwc&token=abc123";
        let sanitized = sanitize_error_for_log(raw);
        assert!(!sanitized.contains("user:pass"));
        assert!(sanitized.contains("sqlite://REDACTED@localhost/tmp/t.db"));
        assert!(!sanitized.contains("abc123"));
        assert!(sanitized.contains("token=REDACTED"));
    }
}
