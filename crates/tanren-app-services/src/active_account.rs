//! Active-account handlers shared by every interface.
//!
//! The caller provides a bounded set of account ids derived from
//! already-validated sessions for the current window/context.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use tanren_contract::{
    AccountFailureReason, ActiveAccountView, ListActiveAccountsResponse, SignedInAccountView,
    SwitchActiveAccountRequest, SwitchActiveAccountResponse,
};
use tanren_identity_policy::AccountId;
use tanren_store::{
    AccountRecord, AccountStore, SwitchActiveAccountAtomicError, SwitchActiveAccountAtomicRequest,
};
use thiserror::Error;

use crate::events::{
    AccountEventKind, ActiveAccountSwitchRejected, ActiveAccountSwitched, envelope,
};
use crate::{AppServiceError, Clock};

/// Active-account context for one caller window/session scope.
///
/// The transport or adapter layer must pass a bounded account-id set
/// produced from validated sessions only.
#[derive(Debug, Clone)]
pub struct ActiveAccountContext {
    /// Account currently active in this caller context.
    active_account_id: AccountId,
    /// Signed-in accounts available for switching in this context.
    signed_in_account_ids: Vec<AccountId>,
}

/// Shared maximum number of signed-in accounts allowed in one active-account
/// registry context.
pub const ACTIVE_ACCOUNT_REGISTRY_LIMIT: usize = 16;

/// Shared maximum number of active-account window entries maintained by
/// interface adapters.
pub const ACTIVE_ACCOUNT_WINDOW_REGISTRY_LIMIT: usize = 16;

/// Validation failures from [`ActiveAccountContext`] construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ActiveAccountContextError {
    /// The caller has no signed-in account ids.
    #[error("signed-in account set must not be empty")]
    EmptySignedInSet,
    /// The active account is not present in the signed-in set.
    #[error("active account is not present in signed-in set")]
    ActiveAccountNotSignedIn,
    /// The signed-in set contains duplicate account ids.
    #[error("signed-in account set contains duplicate account id: {account_id}")]
    DuplicateSignedInAccountId { account_id: AccountId },
    /// The signed-in set exceeds the shared bounded account limit.
    #[error("signed-in account set exceeds limit ({limit}); actual size: {actual}")]
    SignedInAccountRegistryLimitExceeded { limit: usize, actual: usize },
}

impl ActiveAccountContext {
    /// Fallible smart constructor for active-account caller context.
    ///
    /// # Errors
    ///
    /// Returns [`ActiveAccountContextError`] when the signed-in set is empty,
    /// contains duplicates, does not include the active account, or exceeds the
    /// shared registry limit.
    pub fn from_account_ids(
        active_account_id: AccountId,
        signed_in_account_ids: Vec<AccountId>,
    ) -> Result<Self, ActiveAccountContextError> {
        if signed_in_account_ids.is_empty() {
            return Err(ActiveAccountContextError::EmptySignedInSet);
        }
        if signed_in_account_ids.len() > ACTIVE_ACCOUNT_REGISTRY_LIMIT {
            return Err(
                ActiveAccountContextError::SignedInAccountRegistryLimitExceeded {
                    limit: ACTIVE_ACCOUNT_REGISTRY_LIMIT,
                    actual: signed_in_account_ids.len(),
                },
            );
        }

        let mut seen = HashSet::with_capacity(signed_in_account_ids.len());
        for account_id in &signed_in_account_ids {
            if !seen.insert(*account_id) {
                return Err(ActiveAccountContextError::DuplicateSignedInAccountId {
                    account_id: *account_id,
                });
            }
        }

        if !seen.contains(&active_account_id) {
            return Err(ActiveAccountContextError::ActiveAccountNotSignedIn);
        }

        Ok(Self {
            active_account_id,
            signed_in_account_ids,
        })
    }

    #[must_use]
    pub fn active_account_id(&self) -> AccountId {
        self.active_account_id
    }

    #[must_use]
    pub fn signed_in_account_ids(&self) -> &[AccountId] {
        &self.signed_in_account_ids
    }
}

#[derive(Debug, Clone)]
struct ActiveAccountVisibility {
    active_account_id: AccountId,
    signed_in_account_ids: Vec<AccountId>,
}

impl ActiveAccountVisibility {
    fn from_context(context: &ActiveAccountContext) -> Self {
        Self {
            active_account_id: context.active_account_id(),
            signed_in_account_ids: context.signed_in_account_ids().to_vec(),
        }
    }

    fn can_view_switch_target(&self, target_account_id: AccountId) -> bool {
        self.signed_in_account_ids.contains(&target_account_id)
    }

    fn signed_in_account_ids(&self) -> &[AccountId] {
        &self.signed_in_account_ids
    }

    fn signed_in_views(
        accounts: Vec<ActiveAccountView>,
        active_account_id: AccountId,
    ) -> Vec<SignedInAccountView> {
        accounts
            .into_iter()
            .map(|account| SignedInAccountView {
                is_active: account.id == active_account_id,
                account,
            })
            .collect()
    }
}

pub(crate) async fn list_active_accounts<S>(
    store: &S,
    context: &ActiveAccountContext,
) -> Result<ListActiveAccountsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let visibility = ActiveAccountVisibility::from_context(context);
    let accounts = load_signed_in_accounts(store, visibility.signed_in_account_ids()).await?;
    Ok(ListActiveAccountsResponse {
        accounts: ActiveAccountVisibility::signed_in_views(accounts, visibility.active_account_id),
    })
}

pub(crate) async fn switch_active_account<S>(
    store: &S,
    clock: &Clock,
    context: &ActiveAccountContext,
    request: SwitchActiveAccountRequest,
) -> Result<SwitchActiveAccountResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let visibility = ActiveAccountVisibility::from_context(context);
    let now = clock.now();

    if !visibility.can_view_switch_target(request.target_account_id) {
        emit_switch_rejected(store, request.target_account_id, now).await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::TargetAccountNotSignedIn,
        ));
    }

    let accounts = load_signed_in_accounts(store, visibility.signed_in_account_ids()).await?;
    match store
        .switch_active_account_atomic(SwitchActiveAccountAtomicRequest {
            from_account_id: context.active_account_id(),
            to_account_id: request.target_account_id,
            signed_in_account_ids: visibility.signed_in_account_ids().to_vec(),
            now,
            switched_event_payload: envelope(
                AccountEventKind::ActiveAccountSwitched,
                &ActiveAccountSwitched {
                    from_account_id: context.active_account_id(),
                    to_account_id: request.target_account_id,
                    at: now,
                },
            ),
        })
        .await
    {
        Ok(()) => {}
        Err(SwitchActiveAccountAtomicError::EmptySignedInSet) => {
            return Err(AppServiceError::InvalidInput(
                "signed-in account set must not be empty".to_owned(),
            ));
        }
        Err(SwitchActiveAccountAtomicError::ActiveAccountNotSignedIn) => {
            return Err(AppServiceError::InvalidInput(
                "active account is not present in signed-in set".to_owned(),
            ));
        }
        Err(SwitchActiveAccountAtomicError::TargetAccountNotSignedIn) => {
            return Err(AppServiceError::Account(
                AccountFailureReason::TargetAccountNotSignedIn,
            ));
        }
        Err(SwitchActiveAccountAtomicError::Store(err)) => {
            return Err(AppServiceError::Store(err));
        }
    }

    Ok(SwitchActiveAccountResponse {
        active_account_id: request.target_account_id,
        accounts: ActiveAccountVisibility::signed_in_views(accounts, request.target_account_id),
    })
}

async fn load_signed_in_accounts<S>(
    store: &S,
    signed_in_ids: &[AccountId],
) -> Result<Vec<ActiveAccountView>, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .find_accounts_by_ids(signed_in_ids)
        .await?
        .into_iter()
        .map(|record| {
            record
                .map(|row| redact_for_active_account_switcher(&row))
                .ok_or_else(|| {
                    AppServiceError::InvalidInput(
                        "signed-in set contains unknown account id".to_owned(),
                    )
                })
        })
        .collect()
}

fn redact_for_active_account_switcher(record: &AccountRecord) -> ActiveAccountView {
    ActiveAccountView {
        id: record.id,
        display_name: record.display_name.clone(),
        org: record.org_id,
    }
}

async fn emit_switch_rejected<S>(
    store: &S,
    target_account_id: AccountId,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            envelope(
                AccountEventKind::ActiveAccountSwitchRejected,
                &ActiveAccountSwitchRejected {
                    reason: AccountFailureReason::TargetAccountNotSignedIn,
                    target_account_id,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}
