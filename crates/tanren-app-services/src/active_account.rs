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

#[derive(Debug, Clone)]
struct ActiveAccountVisibility {
    active_account_id: AccountId,
    signed_in_account_ids: Vec<AccountId>,
}

impl ActiveAccountVisibility {
    fn from_context(context: &ActiveAccountContext) -> Result<Self, AppServiceError> {
        let signed_in_ids = dedupe_preserving_order(&context.signed_in_account_ids);
        if !signed_in_ids.contains(&context.active_account_id) {
            return Err(AppServiceError::InvalidInput(
                "active account is not present in signed-in set".to_owned(),
            ));
        }
        Ok(Self {
            active_account_id: context.active_account_id,
            signed_in_account_ids: signed_in_ids,
        })
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
    let visibility = ActiveAccountVisibility::from_context(context)?;
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
    let visibility = ActiveAccountVisibility::from_context(context)?;
    let now = clock.now();

    if !visibility.can_view_switch_target(request.target_account_id) {
        emit_switch_rejected(store, request.target_account_id, now).await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::TargetAccountNotSignedIn,
        ));
    }

    let accounts = load_signed_in_accounts(store, visibility.signed_in_account_ids()).await?;
    emit_switched(
        store,
        context.active_account_id,
        request.target_account_id,
        now,
    )
    .await?;

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
    let mut accounts = Vec::with_capacity(signed_in_ids.len());
    for account_id in signed_in_ids {
        if let Some(record) = store.find_account_by_id(*account_id).await? {
            accounts.push(redact_for_active_account_switcher(&record));
        }
    }
    if accounts.len() != signed_in_ids.len() {
        return Err(AppServiceError::InvalidInput(
            "signed-in set contains unknown account id".to_owned(),
        ));
    }
    Ok(accounts)
}

fn redact_for_active_account_switcher(record: &AccountRecord) -> ActiveAccountView {
    ActiveAccountView {
        id: record.id,
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
