use chrono::Utc;
use tanren_app_services::{AppServiceError, OrganizationStore, Store};
use tanren_contract::OrganizationFailureReason;
use tanren_identity_policy::SessionToken;

pub(crate) fn require_token(token: Option<&SessionToken>) -> Result<&str, AppServiceError> {
    token
        .map(SessionToken::expose_secret)
        .filter(|s| !s.is_empty())
        .ok_or(AppServiceError::Organization(
            OrganizationFailureReason::AuthRequired,
        ))
}

pub(crate) async fn resolve_session(
    store: &Store,
    token: &str,
) -> Result<tanren_identity_policy::AccountId, AppServiceError> {
    let now = Utc::now();
    let session = store.resolve_bearer_session(token, now).await?;
    match session {
        Some(s) => Ok(s.account_id),
        None => Err(AppServiceError::Organization(
            OrganizationFailureReason::AuthRequired,
        )),
    }
}
