use chrono::{DateTime, Utc};
use tanren_contract::{
    AccountFailureReason, CreateOrgInvitationRequest, CreateOrgInvitationResponse,
    InvitationStatus, ListOrgInvitationsResponse, OrgInvitationView, RevokeOrgInvitationRequest,
    RevokeOrgInvitationResponse,
};
use tanren_identity_policy::{
    AccountId, Identifier, InvitationToken, OrgId, OrganizationPermission, SessionToken,
};
use tanren_store::{
    AccountStore, ConsumeInvitationError, CreateInvitation, InvitationRecord, RevokeInvitationError,
};

use crate::events::{
    AccountEventKind, InvitationCreated, InvitationRevoked, InviteDenied, PermissionGranted,
    envelope,
};
use crate::{AppServiceError, Clock};

const ADMIN_PERMISSION: &str = "admin";

#[derive(Debug)]
pub struct AcceptExistingInvitationOutput {
    pub org_id: OrgId,
    pub permissions: Vec<OrganizationPermission>,
}

fn is_org_admin(permissions: &[OrganizationPermission]) -> bool {
    permissions.iter().any(|p| p.as_str() == ADMIN_PERMISSION)
}

async fn check_org_admin_permission<S>(
    store: &S,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let permissions = store
        .find_organization_permissions(account_id, org_id)
        .await?;
    if is_org_admin(&permissions) {
        Ok(())
    } else {
        Err(AppServiceError::Account(
            AccountFailureReason::PermissionDenied,
        ))
    }
}

async fn emit_invite_denied<S>(
    store: &S,
    reason: AccountFailureReason,
    org_id: Option<OrgId>,
    attempted_by: Option<AccountId>,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            envelope(
                AccountEventKind::InviteDenied,
                &InviteDenied {
                    reason,
                    org_id,
                    attempted_by,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

fn invitation_view(record: &InvitationRecord) -> OrgInvitationView {
    let status = if record.revoked_at.is_some() {
        InvitationStatus::Revoked
    } else if record.consumed_at.is_some() {
        InvitationStatus::Accepted
    } else {
        InvitationStatus::Pending
    };
    OrgInvitationView {
        token: record.token.clone(),
        org_id: record.inviting_org_id,
        recipient_identifier: record.recipient_identifier.clone(),
        permissions: record.granted_permissions.clone(),
        status,
        creator: record.created_by_account_id,
        created_at: record.created_at,
        expires_at: record.expires_at,
        revoked_at: record.revoked_at,
    }
}

fn is_pending(record: &InvitationRecord) -> bool {
    record.consumed_at.is_none() && record.revoked_at.is_none()
}

pub(crate) async fn create_invitation<S>(
    store: &S,
    clock: &Clock,
    caller_account_id: AccountId,
    caller_org_context: Option<OrgId>,
    request: CreateOrgInvitationRequest,
) -> Result<CreateOrgInvitationResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();

    let Some(org_id) = caller_org_context else {
        emit_invite_denied(
            store,
            AccountFailureReason::PersonalContext,
            Some(request.org_id),
            Some(caller_account_id),
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::PersonalContext,
        ));
    };

    if check_org_admin_permission(store, caller_account_id, org_id)
        .await
        .is_err()
    {
        emit_invite_denied(
            store,
            AccountFailureReason::PermissionDenied,
            Some(request.org_id),
            Some(caller_account_id),
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::PermissionDenied,
        ));
    }

    let token = InvitationToken::parse(SessionToken::generate().expose_secret())
        .map_err(|e| AppServiceError::InvalidInput(e.to_string()))?;
    let created_at = now;
    let expires_at = request.expires_at;

    let invitation = store
        .create_invitation(CreateInvitation {
            token: token.clone(),
            inviting_org_id: request.org_id,
            recipient_identifier: request.recipient_identifier.clone(),
            granted_permissions: request.permissions.clone(),
            created_by_account_id: caller_account_id,
            created_at,
            expires_at,
        })
        .await?;

    store
        .append_event(
            envelope(
                AccountEventKind::InvitationCreated,
                &InvitationCreated {
                    org_id: request.org_id,
                    token,
                    recipient_identifier: request.recipient_identifier,
                    granted_permissions: request.permissions,
                    created_by: caller_account_id,
                    at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(CreateOrgInvitationResponse {
        invitation: invitation_view(&invitation),
    })
}

pub(crate) async fn list_org_invitations<S>(
    store: &S,
    caller_account_id: AccountId,
    org_id: OrgId,
) -> Result<ListOrgInvitationsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    if check_org_admin_permission(store, caller_account_id, org_id)
        .await
        .is_err()
    {
        return Err(AppServiceError::Account(
            AccountFailureReason::PermissionDenied,
        ));
    }

    let records = store.list_invitations_by_org(org_id).await?;
    let invitations = records.iter().map(invitation_view).collect();
    Ok(ListOrgInvitationsResponse { invitations })
}

pub(crate) async fn list_recipient_invitations<S>(
    store: &S,
    identifier: &Identifier,
) -> Result<ListOrgInvitationsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let records = store.list_invitations_by_recipient(identifier).await?;
    let invitations = records
        .iter()
        .filter(|r| is_pending(r))
        .map(invitation_view)
        .collect();
    Ok(ListOrgInvitationsResponse { invitations })
}

pub(crate) async fn revoke_invitation<S>(
    store: &S,
    clock: &Clock,
    caller_account_id: AccountId,
    caller_org_context: Option<OrgId>,
    request: RevokeOrgInvitationRequest,
) -> Result<RevokeOrgInvitationResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();

    let Some(org_id) = caller_org_context else {
        emit_invite_denied(
            store,
            AccountFailureReason::PersonalContext,
            Some(request.org_id),
            Some(caller_account_id),
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::PersonalContext,
        ));
    };

    if check_org_admin_permission(store, caller_account_id, org_id)
        .await
        .is_err()
    {
        emit_invite_denied(
            store,
            AccountFailureReason::PermissionDenied,
            Some(request.org_id),
            Some(caller_account_id),
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::PermissionDenied,
        ));
    }

    let record = store
        .revoke_invitation(&request.token, now)
        .await
        .map_err(|e| match e {
            RevokeInvitationError::NotFound => {
                AppServiceError::Account(AccountFailureReason::InvitationNotFound)
            }
            RevokeInvitationError::AlreadyConsumed => {
                AppServiceError::Account(AccountFailureReason::InvitationAlreadyConsumed)
            }
            RevokeInvitationError::AlreadyRevoked => {
                AppServiceError::Account(AccountFailureReason::InvitationRevoked)
            }
            RevokeInvitationError::Store(s) => AppServiceError::Store(s),
        })?;

    store
        .append_event(
            envelope(
                AccountEventKind::InvitationRevoked,
                &InvitationRevoked {
                    org_id: request.org_id,
                    token: request.token,
                    revoked_by: caller_account_id,
                    at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(RevokeOrgInvitationResponse {
        invitation: invitation_view(&record),
    })
}

pub(crate) async fn accept_invitation_existing<S>(
    store: &S,
    clock: &Clock,
    account_id: AccountId,
    identifier: Identifier,
    token: InvitationToken,
) -> Result<AcceptExistingInvitationOutput, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();

    let invitation = store
        .find_invitation_by_token(&token)
        .await?
        .ok_or_else(|| AppServiceError::Account(AccountFailureReason::InvitationNotFound))?;

    if invitation.revoked_at.is_some() {
        emit_invite_denied(
            store,
            AccountFailureReason::InvitationRevoked,
            Some(invitation.inviting_org_id),
            Some(account_id),
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::InvitationRevoked,
        ));
    }

    if invitation.consumed_at.is_some() {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvitationAlreadyConsumed,
        ));
    }

    if invitation.expires_at <= now {
        return Err(AppServiceError::Account(
            AccountFailureReason::InvitationExpired,
        ));
    }

    if invitation.recipient_identifier != identifier {
        emit_invite_denied(
            store,
            AccountFailureReason::PermissionDenied,
            Some(invitation.inviting_org_id),
            Some(account_id),
            now,
        )
        .await?;
        return Err(AppServiceError::Account(
            AccountFailureReason::PermissionDenied,
        ));
    }

    let consumed = store
        .consume_invitation(&token, now)
        .await
        .map_err(map_consume_error)?;

    let granted_permissions = consumed.granted_permissions.clone();
    let joined_org = consumed.inviting_org_id;

    store
        .insert_membership(account_id, joined_org, granted_permissions.clone(), now)
        .await?;

    store
        .append_event(
            envelope(
                AccountEventKind::PermissionGranted,
                &PermissionGranted {
                    account_id,
                    org_id: joined_org,
                    permissions: granted_permissions.clone(),
                    at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(AcceptExistingInvitationOutput {
        org_id: joined_org,
        permissions: granted_permissions,
    })
}

fn map_consume_error(err: ConsumeInvitationError) -> AppServiceError {
    match err {
        ConsumeInvitationError::NotFound => {
            AppServiceError::Account(AccountFailureReason::InvitationNotFound)
        }
        ConsumeInvitationError::AlreadyConsumed => {
            AppServiceError::Account(AccountFailureReason::InvitationAlreadyConsumed)
        }
        ConsumeInvitationError::Expired => {
            AppServiceError::Account(AccountFailureReason::InvitationExpired)
        }
        ConsumeInvitationError::Revoked => {
            AppServiceError::Account(AccountFailureReason::InvitationRevoked)
        }
        ConsumeInvitationError::Store(s) => AppServiceError::Store(s),
    }
}
