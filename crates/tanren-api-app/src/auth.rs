use axum::response::Response;
use chrono::Utc;
use tanren_app_services::{AccountStore, AppServiceError};
use tanren_contract::AccountFailureReason;
use tanren_identity_policy::{AccountId, SessionToken};
use tower_sessions::Session;

use crate::AppState;
use crate::cookies::{SessionRead, read_cookie_session};
use crate::errors::{map_organization_app_error, organization_internal_error};

#[derive(Clone)]
pub(crate) struct AuthoritativeAuth(pub(crate) AccountId, pub(crate) SessionToken);

pub(crate) async fn require_authoritative_auth(
    state: &AppState,
    session: &Session,
) -> Result<AuthoritativeAuth, Response> {
    let cookie = require_authenticated_session(session).await?;
    resolve_authoritative_auth(state, cookie).await
}

async fn require_authenticated_session(session: &Session) -> Result<SessionRead, Response> {
    match read_cookie_session(session).await {
        Ok(Some(auth)) => Ok(auth),
        Ok(None) => Err(map_organization_app_error(AppServiceError::Account(
            AccountFailureReason::AuthRequired,
        ))),
        Err(err) => {
            tracing::error!(target: "tanren_api", error = %err, "read cookie session");
            Err(organization_internal_error())
        }
    }
}

async fn resolve_authoritative_auth(
    state: &AppState,
    cookie: SessionRead,
) -> Result<AuthoritativeAuth, Response> {
    let canonical = match state.store.find_session_by_token(&cookie.token).await {
        Ok(Some(session)) => session,
        Ok(None) => {
            return Err(map_organization_app_error(AppServiceError::Account(
                AccountFailureReason::AuthRequired,
            )));
        }
        Err(err) => return Err(map_organization_app_error(AppServiceError::Store(err))),
    };

    if canonical.account_id != cookie.account_id
        || canonical.expires_at != cookie.expires_at
        || canonical.expires_at <= Utc::now()
    {
        return Err(map_organization_app_error(AppServiceError::Account(
            AccountFailureReason::AuthRequired,
        )));
    }

    Ok(AuthoritativeAuth(canonical.account_id, canonical.token))
}
