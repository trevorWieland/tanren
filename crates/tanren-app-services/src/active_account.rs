//! Active-account handlers shared by every interface.
//!
//! The caller provides a bounded set of account ids derived from
//! already-validated sessions for the current window/context.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use tanren_contract::{
    AccountFailureReason, AccountView, ListActiveAccountsResponse, SignedInAccountView,
    SwitchActiveAccountRequest, SwitchActiveAccountResponse,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountRecord, AccountStore};

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
    pub active_account_id: AccountId,
    /// Signed-in accounts available for switching in this context.
    pub signed_in_account_ids: Vec<AccountId>,
}

pub(crate) async fn list_active_accounts<S>(
    store: &S,
    context: &ActiveAccountContext,
) -> Result<ListActiveAccountsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let signed_in_ids = dedupe_preserving_order(&context.signed_in_account_ids);
    if !signed_in_ids.contains(&context.active_account_id) {
        return Err(AppServiceError::InvalidInput(
            "active account is not present in signed-in set".to_owned(),
        ));
    }

    let accounts = load_signed_in_accounts(store, &signed_in_ids).await?;
    Ok(ListActiveAccountsResponse {
        accounts: into_signed_in_views(accounts, context.active_account_id),
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
    let signed_in_ids = dedupe_preserving_order(&context.signed_in_account_ids);
    let now = clock.now();

    if !signed_in_ids.contains(&request.target_account_id) {
        emit_switch_rejected(store, request.target_account_id, now).await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::TargetAccountNotSignedIn,
        ));
    }

    let accounts = load_signed_in_accounts(store, &signed_in_ids).await?;
    emit_switched(
        store,
        context.active_account_id,
        request.target_account_id,
        now,
    )
    .await?;

    Ok(SwitchActiveAccountResponse {
        active_account_id: request.target_account_id,
        accounts: into_signed_in_views(accounts, request.target_account_id),
    })
}

async fn load_signed_in_accounts<S>(
    store: &S,
    signed_in_ids: &[AccountId],
) -> Result<Vec<AccountView>, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let mut accounts = Vec::with_capacity(signed_in_ids.len());
    for account_id in signed_in_ids {
        if let Some(record) = store.find_account_by_id(*account_id).await? {
            accounts.push(account_view(&record));
        }
    }
    Ok(accounts)
}

fn account_view(record: &AccountRecord) -> AccountView {
    AccountView {
        id: record.id,
        identifier: record.identifier.clone(),
        display_name: record.display_name.clone(),
        org: record.org_id,
    }
}

fn dedupe_preserving_order(ids: &[AccountId]) -> Vec<AccountId> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(ids.len());
    for account_id in ids {
        if seen.insert(*account_id) {
            out.push(*account_id);
        }
    }
    out
}

fn into_signed_in_views(
    accounts: Vec<AccountView>,
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

async fn emit_switched<S>(
    store: &S,
    from_account_id: AccountId,
    to_account_id: AccountId,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            envelope(
                AccountEventKind::ActiveAccountSwitched,
                &ActiveAccountSwitched {
                    from_account_id,
                    to_account_id,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
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
