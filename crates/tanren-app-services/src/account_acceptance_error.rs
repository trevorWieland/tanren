use chrono::{DateTime, Utc};
use tanren_contract::AccountFailureReason;
use tanren_store::{AcceptInvitationError, AccountStore};

use crate::AppServiceError;

/// Translate store-layer invitation-acceptance failures into contract
/// taxonomy when possible. `SQLite` lock/busy contention is disambiguated
/// with a read of the invitation row so single-request transient lock
/// errors do not get misreported as already-consumed.
pub(crate) async fn map_accept_invitation_error<S>(
    store: &S,
    err: AcceptInvitationError,
    token: &tanren_identity_policy::InvitationToken,
    now: DateTime<Utc>,
) -> Result<AccountFailureReason, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    match err {
        AcceptInvitationError::InvitationNotFound => Ok(AccountFailureReason::InvitationNotFound),
        AcceptInvitationError::InvitationAlreadyConsumed => {
            Ok(AccountFailureReason::InvitationAlreadyConsumed)
        }
        AcceptInvitationError::InvitationExpired => Ok(AccountFailureReason::InvitationExpired),
        AcceptInvitationError::DuplicateIdentifier => Ok(AccountFailureReason::DuplicateIdentifier),
        AcceptInvitationError::Store(store_err) => {
            if is_sqlite_invitation_contention(&store_err) {
                classify_sqlite_invitation_contention(store, store_err, token, now).await
            } else {
                Err(AppServiceError::Store(store_err))
            }
        }
    }
}

pub(crate) fn is_sqlite_invitation_contention(err: &tanren_store::StoreError) -> bool {
    let lower = err.to_string().to_lowercase();
    lower.contains("database is locked")
        || lower.contains("database table is locked")
        || lower.contains("database schema is locked")
        || lower.contains("database is busy")
}

async fn classify_sqlite_invitation_contention<S>(
    store: &S,
    source: tanren_store::StoreError,
    token: &tanren_identity_policy::InvitationToken,
    now: DateTime<Utc>,
) -> Result<AccountFailureReason, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    match store.find_invitation_by_token(token).await {
        Ok(Some(row)) if row.consumed_at.is_some() => {
            Ok(AccountFailureReason::InvitationAlreadyConsumed)
        }
        Ok(Some(row)) if row.expires_at <= now => Ok(AccountFailureReason::InvitationExpired),
        Ok(None) => Ok(AccountFailureReason::InvitationNotFound),
        Ok(Some(_)) => Err(AppServiceError::Store(source)),
        Err(read_err) => {
            if is_sqlite_invitation_contention(&read_err) {
                Ok(AccountFailureReason::InvitationAlreadyConsumed)
            } else {
                Err(AppServiceError::Store(source))
            }
        }
    }
}
