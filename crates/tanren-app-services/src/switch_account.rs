//! Active-account switch handler and window-id validation (B-0046).
//!
//! Validates that the requested target account is present in the
//! caller-supplied signed-in set, emits the appropriate event, and
//! returns the new active [`AccountView`]. Rejected switches (target not
//! in signed-in set) also emit an `active_account_switch_rejected` event
//! so audit consumers can observe all attempts.

use tanren_contract::{AccountFailureReason, AccountView, SwitchActiveAccountResponse};
use tanren_identity_policy::AccountId;
use tanren_store::AccountStore;

use crate::events::{
    AccountEventKind, ActiveAccountSwitchRejected, ActiveAccountSwitched, envelope,
};
use crate::{AppServiceError, Clock};

/// Execute the active-account switch for the supplied signed-in set and
/// target.
///
/// `signed_in_accounts` must be the ordered list of account ids that the
/// caller's session currently considers signed-in (derived from the
/// tower-sessions `signed_in_accounts` key for the `@api` surface, or
/// maintained in-memory for in-process harnesses). The caller is
/// responsible for reading and updating the session; this handler only
/// validates, emits events, and returns the new active view.
///
/// # Errors
///
/// - [`AppServiceError::Account`] with
///   [`AccountFailureReason::TargetAccountNotSignedIn`] when `target` is
///   not in `signed_in_accounts`.
/// - [`AppServiceError::Store`] for unexpected database failures.
pub(crate) async fn switch_active_account<S>(
    store: &S,
    clock: &Clock,
    signed_in_accounts: &[AccountId],
    target: AccountId,
    window_id: Option<String>,
) -> Result<SwitchActiveAccountResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();

    if !signed_in_accounts.contains(&target) {
        store
            .append_event(
                envelope(
                    AccountEventKind::ActiveAccountSwitchRejected,
                    &ActiveAccountSwitchRejected {
                        requested_target: target,
                        window_id: window_id.clone(),
                        reason: AccountFailureReason::TargetAccountNotSignedIn,
                        at: now,
                    },
                ),
                now,
            )
            .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::TargetAccountNotSignedIn,
        ));
    }

    let account_record = store
        .find_account_by_id(target)
        .await?
        .ok_or_else(|| AppServiceError::InvalidInput("target account not found".to_owned()))?;

    store
        .append_event(
            envelope(
                AccountEventKind::ActiveAccountSwitched,
                &ActiveAccountSwitched {
                    switched_to: target,
                    window_id: window_id.clone(),
                    at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(SwitchActiveAccountResponse {
        active_account: AccountView {
            id: account_record.id,
            identifier: account_record.identifier,
            display_name: account_record.display_name,
            org: account_record.org_id,
        },
    })
}
