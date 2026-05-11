use std::io::Read;

use tanren_app_services::{AccountStore, Clock, Store};
use tanren_contract::ProjectFailureCode;
use tanren_identity_policy::{AccountId, DesignatedHost, RepositoryRef, SessionToken};
use tracing::error;

use super::{ProjectCommandResult, ProjectFailureBody};

pub(super) fn parse_account_id(raw: &str) -> ProjectCommandResult<AccountId> {
    AccountId::parse(raw).map_err(|_| {
        ProjectFailureBody::validation("The owning_account_id must be a valid UUIDv7.")
    })
}

pub(super) fn parse_repository_ref(raw: &str) -> ProjectCommandResult<RepositoryRef> {
    RepositoryRef::parse(raw).map_err(|_| {
        ProjectFailureBody::validation("The repository must be provided as owner/name.")
    })
}

pub(super) fn parse_designated_host(raw: &str) -> ProjectCommandResult<DesignatedHost> {
    DesignatedHost::parse(raw).map_err(|_| {
        ProjectFailureBody::validation("The designated_host must be a valid provider host key.")
    })
}

fn parse_session_token(raw: &str) -> ProjectCommandResult<SessionToken> {
    let token = raw.trim();
    SessionToken::parse(token).map_err(|_| {
        ProjectFailureBody::validation(
            "The session_token must be a valid base64url-no-pad session bearer.",
        )
    })
}

fn read_session_token_from_stdin() -> ProjectCommandResult<SessionToken> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).map_err(|err| {
        error!(error = ?err, "project command failed to read session token from stdin");
        ProjectFailureBody::internal(
            "Tanren encountered an internal error while processing the project request.",
        )
    })?;
    parse_session_token(&input)
}

fn load_session_token_from_env() -> ProjectCommandResult<Option<SessionToken>> {
    let raw = match std::env::var("TANREN_SESSION_TOKEN") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(err) => {
            error!(error = ?err, "project command failed to read session token environment variable");
            return Err(ProjectFailureBody::internal(
                "Tanren encountered an internal error while processing the project request.",
            ));
        }
    };
    if raw.trim().is_empty() {
        return Ok(None);
    }
    parse_session_token(&raw).map(Some)
}

fn load_default_session_token() -> ProjectCommandResult<Option<SessionToken>> {
    let path = super::super::session_path();
    match std::fs::read_to_string(&path) {
        Ok(raw) => parse_session_token(&raw).map(Some),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => {
            error!(error = ?err, path = %path.display(), "project command failed to read session token");
            Err(ProjectFailureBody::internal(
                "Tanren encountered an internal error while processing the project request.",
            ))
        }
    }
}

pub(super) async fn resolve_actor_account_id(
    store: &Store,
    session_token_stdin: bool,
) -> ProjectCommandResult<AccountId> {
    let token = if session_token_stdin {
        read_session_token_from_stdin()?
    } else if let Some(token) = load_session_token_from_env()? {
        token
    } else {
        load_default_session_token()?.ok_or_else(|| ProjectFailureBody {
            code: ProjectFailureCode::AuthRequired,
            summary: "Sign in first or provide TANREN_SESSION_TOKEN or --session-token-stdin."
                .to_owned(),
        })?
    };
    let session = store
        .find_active_session(&token, Clock::default().now())
        .await
        .map_err(|err| {
            error!(error = ?err, "project command failed to resolve session");
            ProjectFailureBody::internal(
                "Tanren encountered an internal error while processing the project request.",
            )
        })?;
    let Some(session) = session else {
        return Err(ProjectFailureBody {
            code: ProjectFailureCode::AuthRequired,
            summary: "The supplied session token is missing or expired.".to_owned(),
        });
    };
    Ok(session.account_id)
}
