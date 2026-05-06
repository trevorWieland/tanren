//! Existing-account join handler: an authenticated account accepts an
//! invitation to join an organization.
//!
//! Delegates to the store's `accept_existing_invitation_atomic` path which
//! runs consume + insert membership + append events in one DB transaction.
//! The account's other organization memberships are unaffected. Project
//! access is NOT granted automatically (M-0031 owns project-level grants).

use tanren_contract::{
    AccountFailureReason, JoinOrganizationRequest, JoinOrganizationResponse, OrgMembershipView,
};
use tanren_identity_policy::{AccountId, MembershipId, OrgPermissions};
use tanren_store::{
    AcceptExistingInvitationError, AcceptExistingInvitationEventContext,
    AcceptExistingInvitationRequest, AccountStore,
};

use crate::events::{AccountEventKind, JoinFailed, OrganizationJoined, envelope};
use crate::{AppServiceError, Clock};

pub(crate) async fn join_organization<S>(
    store: &S,
    clock: &Clock,
    account_id: AccountId,
    request: JoinOrganizationRequest,
) -> Result<JoinOrganizationResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let token = request.invitation_token.clone();

    let account = store
        .find_account_by_id(account_id)
        .await?
        .ok_or_else(|| AppServiceError::Account(AccountFailureReason::Unauthenticated))?;

    let outcome = match store
        .accept_existing_invitation_atomic(AcceptExistingInvitationRequest {
            token: token.clone(),
            account_id,
            identifier: account.identifier.clone(),
            membership_id: MembershipId::fresh(),
            now,
            events_builder: Box::new(
                |ctx: &AcceptExistingInvitationEventContext| -> Vec<serde_json::Value> {
                    vec![envelope(
                        AccountEventKind::OrganizationJoined,
                        &OrganizationJoined {
                            account_id: ctx.account_id,
                            joined_org: ctx.joined_org,
                            at: ctx.now,
                        },
                    )]
                },
            ),
        })
        .await
    {
        Ok(o) => o,
        Err(err) => {
            let reason = map_join_error(&err);
            store
                .append_event(
                    envelope(
                        AccountEventKind::JoinFailed,
                        &JoinFailed {
                            reason,
                            account_id,
                            token: token.clone(),
                            at: now,
                        },
                    ),
                    now,
                )
                .await?;
            return Err(AppServiceError::Account(reason));
        }
    };

    let membership_permissions = outcome
        .membership
        .org_permissions
        .clone()
        .unwrap_or_else(OrgPermissions::member);

    let selectable_organizations = store
        .list_memberships_for_account(account_id)
        .await?
        .into_iter()
        .map(|m| OrgMembershipView {
            org_id: m.org_id,
            permissions: m.org_permissions.unwrap_or_else(OrgPermissions::member),
        })
        .collect();

    Ok(JoinOrganizationResponse {
        joined_org: outcome.joined_org,
        membership_permissions,
        selectable_organizations,
        project_access_grants: vec![],
    })
}

fn map_join_error(err: &AcceptExistingInvitationError) -> AccountFailureReason {
    match err {
        AcceptExistingInvitationError::InvitationNotFound => {
            AccountFailureReason::InvitationNotFound
        }
        AcceptExistingInvitationError::InvitationAlreadyConsumed
        | AcceptExistingInvitationError::InvitationRevoked
        | AcceptExistingInvitationError::AlreadyMember => {
            AccountFailureReason::InvitationAlreadyConsumed
        }
        AcceptExistingInvitationError::InvitationExpired => AccountFailureReason::InvitationExpired,
        AcceptExistingInvitationError::WrongAccount => AccountFailureReason::WrongAccount,
        AcceptExistingInvitationError::Store(_) => AccountFailureReason::ValidationFailed,
    }
}
