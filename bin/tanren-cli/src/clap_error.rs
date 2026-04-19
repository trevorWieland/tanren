//! Transport-safe mapping from clap parser failures to contract error
//! responses.
//!
//! The goal is a stable, redacted wire response for CLI parse errors.
//! Clap's `to_string()` prose is version-sensitive and can echo raw
//! user input (including values typed into the wrong slot, which may
//! contain secrets). This module takes two guarantees to avoid that:
//!
//! 1. `clap::error::ErrorKind` is classified into a stable
//!    [`CliParseReasonCode`]. The wire `reason_code` never changes
//!    across clap versions.
//! 2. Any reported argument name is passed through
//!    [`ALLOWED_ARG_FIELDS`]. Anything not on the allowlist is dropped
//!    to `None` — raw user text never reaches the wire payload.

use clap::error::{ContextKind, ContextValue, ErrorKind};
use tanren_contract::{CliParseReasonCode, ContractError, ErrorResponse};

/// Long-flag names that are safe to echo into wire error details.
///
/// The unit test `allowed_arg_fields_covers_every_declared_long_flag`
/// in `main.rs` asserts that every `long` on our clap declaration is
/// represented here. Anything outside this list is dropped when
/// clap's `InvalidArg` context returns it.
pub(crate) const ALLOWED_ARG_FIELDS: &[&str] = &[
    // Global flags.
    "database_url",
    "log_level",
    "actor_token_stdin",
    "actor_token_file",
    "actor_public_key_file",
    "token_issuer",
    "token_audience",
    "actor_token_max_ttl_secs",
    // Dispatch subcommand flags.
    "project",
    "phase",
    "cli",
    "branch",
    "spec_folder",
    "workflow_id",
    "mode",
    "timeout",
    "environment_profile",
    "auth_mode",
    "gate_cmd",
    "context",
    "model",
    "project_env",
    "required_secret",
    "preserve_on_failure",
    "id",
    "status",
    "lane",
    "limit",
    "cursor",
    "reason",
    // `db purge-replay` subcommand flags.
    "batch_limit",
    "retention_secs",
    // `install` subcommand flags.
    "config",
    "dry_run",
    "strict",
    "profile",
    "source",
    "target",
    // `methodology` subcommand flags.
    "json",
    "params_file",
    "params_stdin",
    "spec_id",
    "agent_session_id",
    "methodology_config",
    "allow_legacy_provenance",
];

/// Translate a clap parser failure into a transport-safe
/// [`ErrorResponse`] without echoing raw user input.
pub(crate) fn clap_error_to_response(err: &clap::Error) -> ErrorResponse {
    let reason_code = classify_clap_error_kind(err.kind());
    let field = allowlisted_field_from_clap_error(err);
    ErrorResponse::from(ContractError::InvalidArgs { field, reason_code })
}

pub(crate) fn classify_clap_error_kind(kind: ErrorKind) -> CliParseReasonCode {
    match kind {
        ErrorKind::MissingRequiredArgument => CliParseReasonCode::MissingRequiredArgument,
        ErrorKind::InvalidValue => CliParseReasonCode::InvalidValue,
        ErrorKind::InvalidSubcommand => CliParseReasonCode::InvalidSubcommand,
        ErrorKind::UnknownArgument => CliParseReasonCode::UnknownArgument,
        ErrorKind::ArgumentConflict => CliParseReasonCode::ArgumentConflict,
        ErrorKind::TooManyValues => CliParseReasonCode::TooManyValues,
        ErrorKind::TooFewValues | ErrorKind::WrongNumberOfValues => {
            CliParseReasonCode::TooFewValues
        }
        ErrorKind::ValueValidation => CliParseReasonCode::ValueValidation,
        ErrorKind::NoEquals => CliParseReasonCode::NoEquals,
        _ => CliParseReasonCode::Format,
    }
}

fn allowlisted_field_from_clap_error(err: &clap::Error) -> Option<String> {
    let raw = err
        .get(ContextKind::InvalidArg)
        .and_then(|value| match value {
            ContextValue::String(name) => Some(name.as_str()),
            ContextValue::Strings(names) => names.first().map(String::as_str),
            _ => None,
        })?;
    let normalized = normalize_clap_arg_name(raw)?;
    if ALLOWED_ARG_FIELDS.contains(&normalized.as_str()) {
        Some(normalized)
    } else {
        None
    }
}

/// Normalize a clap-reported `InvalidArg` string to the canonical
/// `snake_case` id used in [`ALLOWED_ARG_FIELDS`]. Clap reports long
/// flags as `"--phase"`, `"--phase <PHASE>"`, or positional ids like
/// `"PROJECT"`. Anything we can't normalize is dropped (`None`) so no
/// untrusted prose ever reaches the wire.
pub(crate) fn normalize_clap_arg_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let head = trimmed.split_whitespace().next()?;
    let stripped = head.trim_start_matches('-');
    if stripped.is_empty() {
        return None;
    }
    if !stripped
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return None;
    }
    Some(stripped.replace('-', "_").to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use tanren_contract::CliParseReasonCode;

    use super::{classify_clap_error_kind, normalize_clap_arg_name};

    #[test]
    fn classify_clap_error_kind_covers_common_variants() {
        assert_eq!(
            classify_clap_error_kind(clap::error::ErrorKind::MissingRequiredArgument),
            CliParseReasonCode::MissingRequiredArgument
        );
        assert_eq!(
            classify_clap_error_kind(clap::error::ErrorKind::InvalidValue),
            CliParseReasonCode::InvalidValue
        );
        assert_eq!(
            classify_clap_error_kind(clap::error::ErrorKind::InvalidSubcommand),
            CliParseReasonCode::InvalidSubcommand
        );
        assert_eq!(
            classify_clap_error_kind(clap::error::ErrorKind::UnknownArgument),
            CliParseReasonCode::UnknownArgument
        );
        assert_eq!(
            classify_clap_error_kind(clap::error::ErrorKind::ArgumentConflict),
            CliParseReasonCode::ArgumentConflict
        );
        assert_eq!(
            classify_clap_error_kind(clap::error::ErrorKind::ValueValidation),
            CliParseReasonCode::ValueValidation
        );
        assert_eq!(
            classify_clap_error_kind(clap::error::ErrorKind::NoEquals),
            CliParseReasonCode::NoEquals
        );
    }

    #[test]
    fn normalize_clap_arg_name_handles_long_flags_and_positionals() {
        assert_eq!(
            normalize_clap_arg_name("--actor-token-stdin").as_deref(),
            Some("actor_token_stdin")
        );
        assert_eq!(
            normalize_clap_arg_name("--phase <PHASE>").as_deref(),
            Some("phase")
        );
        assert_eq!(
            normalize_clap_arg_name("PROJECT").as_deref(),
            Some("project")
        );
        assert_eq!(
            normalize_clap_arg_name("--with-dash-in-name").as_deref(),
            Some("with_dash_in_name")
        );
        assert!(normalize_clap_arg_name("--").is_none());
        assert!(normalize_clap_arg_name("").is_none());
        // Reject values that contain anything outside the allowed
        // character class so clap can't inject prose into the wire.
        assert!(normalize_clap_arg_name("--phase=not valid").is_none());
    }
}
