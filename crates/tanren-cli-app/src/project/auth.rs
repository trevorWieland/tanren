use secrecy::SecretString;
use tanren_app_services::{AccountStore, Clock, Store};
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
    if token.is_empty() {
        return Err(ProjectFailureBody::validation(
            "The session_token cannot be empty.",
        ));
    }
    Ok(SessionToken::from_secret(SecretString::from(
        token.to_owned(),
    )))
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
    session_token_raw: Option<&str>,
) -> ProjectCommandResult<AccountId> {
    let token = match session_token_raw {
        Some(raw) => parse_session_token(raw)?,
        None => load_default_session_token()?.ok_or_else(|| ProjectFailureBody {
            code: "auth_required".to_owned(),
            summary: "Sign in first or provide --session-token.".to_owned(),
        })?,
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
            code: "auth_required".to_owned(),
            summary: "The supplied session token is missing or expired.".to_owned(),
        });
    };
    Ok(session.account_id)
}
