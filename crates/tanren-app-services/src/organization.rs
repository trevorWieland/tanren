use tanren_contract::{
    CreateOrganizationRequest, OrganizationAdminOperation, OrganizationFailureReason,
    OrganizationMembershipView, OrganizationView,
};
use tanren_identity_policy::{AccountId, MembershipId, OrgId, OrgPermission, OrganizationName};
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
    pub organization: OrganizationView,
    pub membership: OrganizationMembershipView,
    pub granted_permissions: Vec<OrgPermission>,
    pub project_count: u64,
    pub available_organizations: Vec<OrganizationView>,
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
    let idempotency_key = request
        .idempotency_key
        .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());

    let org_id = OrgId::fresh();
    let membership_id = MembershipId::fresh();

    let event_payload = envelope(
        OrganizationEventKind::OrganizationCreated,
        &OrganizationCreatedEvent {
            org_id,
            creator_account_id: account_id,
            canonical_name: canonical_name.clone(),
            granted_permissions: BOOTSTRAP_PERMISSIONS.to_vec(),
            at: now,
        },
    );

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
            request_id: idempotency_key,
            event_payload,
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
        Err(CreateOrganizationError::IdempotencyConflict) => {
            return Err(AppServiceError::Organization(
                OrganizationFailureReason::IdempotencyConflict,
            ));
        }
        Err(CreateOrganizationError::Store(err)) => return Err(AppServiceError::Store(err)),
    };

    let available_organizations = store.list_account_organizations(account_id).await?;
    let available_organizations = convert_org_list(available_organizations)?;

    Ok(CreateOrganizationOutput {
        organization: org_record_to_view(&outcome.organization)?,
        membership: membership_record_to_view(&outcome.membership, BOOTSTRAP_PERMISSIONS.to_vec()),
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
) -> Result<Vec<OrganizationView>, AppServiceError>
where
    S: OrganizationStore + ?Sized,
{
    let orgs = store.list_account_organizations(account_id).await?;
    convert_org_list(orgs)
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

fn org_record_to_view(record: &OrganizationRecord) -> Result<OrganizationView, AppServiceError> {
    let name = OrganizationName::parse(&record.canonical_name).map_err(|_| {
        AppServiceError::InvalidInput(format!(
            "data integrity: canonical_name {:?} failed re-validation",
            record.canonical_name
        ))
    })?;
    Ok(OrganizationView {
        id: record.id,
        name,
        created_at: record.created_at,
    })
}

fn membership_record_to_view(
    record: &MembershipRecord,
    permissions: Vec<OrgPermission>,
) -> OrganizationMembershipView {
    OrganizationMembershipView {
        id: record.id,
        account_id: record.account_id,
        org_id: record.org_id,
        permissions,
        created_at: record.created_at,
    }
}

fn convert_org_list(
    records: Vec<OrganizationRecord>,
) -> Result<Vec<OrganizationView>, AppServiceError> {
    records
        .into_iter()
        .map(|r| org_record_to_view(&r))
        .collect()
}
