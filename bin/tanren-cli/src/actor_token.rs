//! Actor-token transport resolution: stdin / file / env with strict
//! one-of semantics, UTF-8 validation, and defense-in-depth size
//! limiting.
//!
//! The authoritative token-size guard lives inside
//! [`tanren_app_services::ActorTokenVerifier::verify`]; the checks
//! here are the first line of defense against OOM / unbounded IO
//! before the verifier is constructed.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use tanren_app_services::ActorTokenVerifier;
use tanren_app_services::auth::DEFAULT_ACTOR_TOKEN_MAX_BYTES;
use tanren_contract::{ContractError, ErrorResponse};
use tanren_observability::emit_correlated_internal_error;
use uuid::Uuid;

pub(crate) const ACTOR_TOKEN_ENV_VAR: &str = "TANREN_ACTOR_TOKEN";

pub(crate) fn resolve_actor_token_verifier(
    actor_public_key_file: Option<&PathBuf>,
    token_issuer: Option<&str>,
    token_audience: Option<&str>,
    actor_token_max_ttl_secs: u64,
) -> Result<ActorTokenVerifier, anyhow::Error> {
    let issuer = token_issuer.ok_or_else(|| {
        anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
            field: "token_issuer".to_owned(),
            reason: "missing required --token-issuer".to_owned(),
        }))
    })?;
    let audience = token_audience.ok_or_else(|| {
        anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
            field: "token_audience".to_owned(),
            reason: "missing required --token-audience".to_owned(),
        }))
    })?;
    if actor_token_max_ttl_secs == 0 {
        return Err(anyhow::Error::new(ErrorResponse::from(
            ContractError::InvalidField {
                field: "actor_token_max_ttl_secs".to_owned(),
                reason: "must be >= 1".to_owned(),
            },
        )));
    }

    if let Some(path) = actor_public_key_file {
        return ActorTokenVerifier::from_public_key_file(
            path,
            issuer,
            audience,
            actor_token_max_ttl_secs,
            DEFAULT_ACTOR_TOKEN_MAX_BYTES,
        )
        .map_err(|err| anyhow::Error::new(ErrorResponse::from(err)));
    }

    Err(anyhow::Error::new(ErrorResponse::from(
        ContractError::InvalidField {
            field: "actor_public_key".to_owned(),
            reason: "missing required --actor-public-key-file".to_owned(),
        },
    )))
}

pub(crate) fn resolve_actor_token(
    actor_token_stdin: bool,
    actor_token_file: Option<&PathBuf>,
) -> Result<String, anyhow::Error> {
    let env_token = std::env::var(ACTOR_TOKEN_ENV_VAR)
        .ok()
        .filter(|token| !token.trim().is_empty());
    let selected_source_count = u8::from(actor_token_stdin)
        + u8::from(actor_token_file.is_some())
        + u8::from(env_token.is_some());

    if selected_source_count > 1 {
        return Err(anyhow::Error::new(ErrorResponse::from(
            ContractError::InvalidField {
                field: "actor_token".to_owned(),
                reason: format!(
                    "exactly one token source is allowed; choose one of --actor-token-stdin, --actor-token-file, or {ACTOR_TOKEN_ENV_VAR}"
                ),
            },
        )));
    }

    if actor_token_stdin {
        let token = read_actor_token_from_stdin()?;
        return normalize_actor_token(&token);
    }

    if let Some(path) = actor_token_file {
        let token = read_actor_token_from_file(path)?;
        return normalize_actor_token(&token);
    }

    if let Some(token) = env_token {
        return normalize_actor_token(&token);
    }

    Err(anyhow::Error::new(ErrorResponse::from(
        ContractError::InvalidField {
            field: "actor_token".to_owned(),
            reason: format!(
                "missing actor token source (use --actor-token-stdin, --actor-token-file, or {ACTOR_TOKEN_ENV_VAR})"
            ),
        },
    )))
}

fn read_actor_token_from_file(path: &Path) -> Result<String, anyhow::Error> {
    let mut file = std::fs::File::open(path).map_err(|err| {
        actor_token_source_error(
            "invalid_actor_token_source",
            &format!(
                "failed to read actor token file `{}`: {err}",
                path.display()
            ),
        )
    })?;
    read_actor_token_from_reader_with_source(&mut file, &format!("file `{}`", path.display()))
}

fn read_actor_token_from_stdin() -> Result<String, anyhow::Error> {
    let mut stdin = std::io::stdin();
    read_actor_token_from_reader_with_source(&mut stdin, "stdin")
}

fn read_actor_token_from_reader_with_source(
    reader: &mut dyn std::io::Read,
    source: &str,
) -> Result<String, anyhow::Error> {
    let mut limited = reader.take((DEFAULT_ACTOR_TOKEN_MAX_BYTES as u64).saturating_add(1));
    let mut token_bytes = Vec::new();
    limited.read_to_end(&mut token_bytes).map_err(|err| {
        actor_token_source_error(
            "invalid_actor_token_source",
            &format!("failed to read actor token from {source}: {err}"),
        )
    })?;
    if token_bytes.len() > DEFAULT_ACTOR_TOKEN_MAX_BYTES {
        return Err(actor_token_source_error(
            "invalid_actor_token_source",
            &format!("actor token from {source} exceeds {DEFAULT_ACTOR_TOKEN_MAX_BYTES} bytes"),
        ));
    }
    String::from_utf8(token_bytes).map_err(|err| {
        actor_token_source_error(
            "invalid_actor_token_source",
            &format!("actor token from {source} is not valid utf-8: {err}"),
        )
    })
}

fn actor_token_source_error(error_code: &str, raw_error: &str) -> anyhow::Error {
    let _ = emit_correlated_internal_error("tanren-cli", error_code, Uuid::now_v7(), raw_error);
    anyhow::Error::new(ErrorResponse::from(ContractError::InvalidField {
        field: "actor_token".to_owned(),
        reason: "invalid actor token source".to_owned(),
    }))
}

fn normalize_actor_token(token: &str) -> Result<String, anyhow::Error> {
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err(anyhow::Error::new(ErrorResponse::from(
            ContractError::InvalidField {
                field: "actor_token".to_owned(),
                reason: "actor token source resolved to an empty token".to_owned(),
            },
        )));
    }
    if token.len() > DEFAULT_ACTOR_TOKEN_MAX_BYTES {
        return Err(actor_token_source_error(
            "invalid_actor_token_source",
            &format!(
                "actor token exceeds {DEFAULT_ACTOR_TOKEN_MAX_BYTES} bytes after normalization"
            ),
        ));
    }

    Ok(token)
}
