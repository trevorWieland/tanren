use tanren_contract::{
    CreateOrganizationRequest, OrganizationAdminOperation, OrganizationFailureReason,
};
use tanren_identity_policy::{AccountId, MembershipId, OrgId, OrgPermission};
use tanren_store::{
    AccountStore, CreateOrganizationAtomicRequest, CreateOrganizationError, MembershipRecord,
    NewOrganization, OrganizationRecord, OrganizationStore,
};

use crate::organization_events::{
    OrganizationCreatedEvent, OrganizationCreationRejectedEvent, OrganizationEventKind, envelope,
};
use crate::{AppServiceError, Clock};

const BOOTSTRAP_PERMISSIONS: [OrgPermission; 5] = [
    OrgPermission::InviteMembers,
    OrgPermission::ManageAccess,
    OrgPermission::Configure,
    OrgPermission::SetPolicy,
    OrgPermission::Delete,
];

#[derive(Debug)]
pub struct CreateOrganizationOutput {
    pub organization: OrganizationRecord,
    pub membership: MembershipRecord,
    pub granted_permissions: Vec<OrgPermission>,
    pub project_count: u64,
    pub available_organizations: Vec<OrganizationRecord>,
}

pub(crate) async fn create_organization_for_account<S>(
    store: &S,
    clock: &Clock,
    account_id: AccountId,
    request: CreateOrganizationRequest,
) -> Result<CreateOrganizationOutput, AppServiceError>
where
    S: AccountStore + OrganizationStore + ?Sized,
{
    let now = clock.now();
    let canonical_name = request.name.as_str().to_owned();

    let org_id = OrgId::fresh();
    let membership_id = MembershipId::fresh();

    let outcome = match store
        .create_organization(CreateOrganizationAtomicRequest {
            organization: NewOrganization {
                id: org_id,
                canonical_name: canonical_name.clone(),
                display_name: canonical_name.clone(),
                creator_account_id: account_id,
                created_at: now,
            },
            membership_id,
            bootstrap_permissions: BOOTSTRAP_PERMISSIONS.to_vec(),
            now,
        })
        .await
    {
        Ok(o) => o,
        Err(CreateOrganizationError::DuplicateName) => {
            store
                .append_event(
                    envelope(
                        OrganizationEventKind::OrganizationCreationRejected,
                        &OrganizationCreationRejectedEvent {
                            reason: OrganizationFailureReason::DuplicateOrganizationName,
                            creator_account_id: account_id,
                            attempted_name: canonical_name,
                            at: now,
                        },
                    ),
                    now,
                )
                .await?;
            return Err(AppServiceError::Organization(
                OrganizationFailureReason::DuplicateOrganizationName,
            ));
        }
        Err(CreateOrganizationError::Store(err)) => return Err(AppServiceError::Store(err)),
    };

    store
        .append_event(
            envelope(
                OrganizationEventKind::OrganizationCreated,
                &OrganizationCreatedEvent {
                    org_id: outcome.organization.id,
                    creator_account_id: account_id,
                    canonical_name: outcome.organization.canonical_name.clone(),
                    granted_permissions: BOOTSTRAP_PERMISSIONS.to_vec(),
                    at: now,
                },
            ),
            now,
        )
        .await?;

    let available_organizations = store.list_account_organizations(account_id).await?;

    Ok(CreateOrganizationOutput {
        organization: outcome.organization,
        membership: outcome.membership,
        granted_permissions: BOOTSTRAP_PERMISSIONS.to_vec(),
        project_count: 0,
        available_organizations,
    })
}

pub(crate) async fn create_organization_with_session<S>(
    store: &S,
    clock: &Clock,
    bearer_token: &str,
    request: CreateOrganizationRequest,
) -> Result<CreateOrganizationOutput, AppServiceError>
where
    S: AccountStore + OrganizationStore + ?Sized,
{
    let now = clock.now();
    let session = store.resolve_bearer_session(bearer_token, now).await?;
    let account_id = match session {
        Some(s) => s.account_id,
        None => {
            return Err(AppServiceError::Organization(
                OrganizationFailureReason::AuthRequired,
            ));
        }
    };
    create_organization_for_account(store, clock, account_id, request).await
}

pub(crate) async fn list_account_organizations<S>(
    store: &S,
    account_id: AccountId,
) -> Result<Vec<OrganizationRecord>, AppServiceError>
where
    S: OrganizationStore + ?Sized,
{
    let orgs = store.list_account_organizations(account_id).await?;
    Ok(orgs)
}

pub(crate) async fn authorize_org_admin_operation<S>(
    store: &S,
    account_id: AccountId,
    org_id: OrgId,
    operation: OrganizationAdminOperation,
) -> Result<(), AppServiceError>
where
    S: OrganizationStore + ?Sized,
{
    let membership = store.find_membership(account_id, org_id).await?;
    if membership.is_none() {
        return Err(AppServiceError::Organization(
            OrganizationFailureReason::PermissionDenied,
        ));
    }
    let Some(permission) = admin_operation_to_permission(operation) else {
        return Err(AppServiceError::Organization(
            OrganizationFailureReason::PermissionDenied,
        ));
    };
    let has = store
        .has_organization_permission(org_id, account_id, permission)
        .await?;
    if has {
        Ok(())
    } else {
        Err(AppServiceError::Organization(
            OrganizationFailureReason::PermissionDenied,
        ))
    }
}

pub(crate) async fn assert_not_last_admin_holder<S>(
    store: &S,
    org_id: OrgId,
    account_id: AccountId,
    permission: OrgPermission,
) -> Result<(), AppServiceError>
where
    S: OrganizationStore + ?Sized,
{
    let count = store.count_permission_holders(org_id, permission).await?;
    if count <= 1 {
        let has = store
            .has_organization_permission(org_id, account_id, permission)
            .await?;
        if has {
            return Err(AppServiceError::Organization(
                OrganizationFailureReason::LastAdminHolder,
            ));
        }
    }
    Ok(())
}

pub(crate) fn admin_operation_to_permission(
    operation: OrganizationAdminOperation,
) -> Option<OrgPermission> {
    match operation {
        OrganizationAdminOperation::InviteMembers => Some(OrgPermission::InviteMembers),
        OrganizationAdminOperation::ManageAccess => Some(OrgPermission::ManageAccess),
        OrganizationAdminOperation::Configure => Some(OrgPermission::Configure),
        OrganizationAdminOperation::SetPolicy => Some(OrgPermission::SetPolicy),
        OrganizationAdminOperation::Delete => Some(OrgPermission::Delete),
        _ => None,
    }
}
