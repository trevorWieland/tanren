use tanren_contract::{
    CreateOrganizationRequest, DEFAULT_ORG_PAGE_SIZE, ListOrganizationsRequest,
    ListOrganizationsResponse, OrganizationAdminOperation, OrganizationFailureReason,
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
    let bootstrap = tanren_identity_policy::bootstrap_permissions();

    let event_payload = envelope(
        OrganizationEventKind::OrganizationCreated,
        &OrganizationCreatedEvent {
            org_id,
            creator_account_id: account_id,
            canonical_name: canonical_name.clone(),
            granted_permissions: bootstrap.to_vec(),
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
            bootstrap_permissions: bootstrap.to_vec(),
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

    let available_organizations = store
        .list_account_organizations(account_id, DEFAULT_ORG_PAGE_SIZE, None)
        .await?;
    let available_organizations = convert_org_list(available_organizations)?;

    Ok(CreateOrganizationOutput {
        organization: org_record_to_view(&outcome.organization)?,
        membership: membership_record_to_view(&outcome.membership, bootstrap.to_vec()),
        granted_permissions: bootstrap.to_vec(),
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
    request: ListOrganizationsRequest,
) -> Result<ListOrganizationsResponse, AppServiceError>
where
    S: OrganizationStore + ?Sized,
{
    let limit = request.limit.unwrap_or(DEFAULT_ORG_PAGE_SIZE);
    let after = request
        .after
        .as_deref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .map(OrgId::new);
    let records = store
        .list_account_organizations(account_id, limit, after)
        .await?;
    let has_more = records.len() > limit as usize;
    let truncated: Vec<_> = records.into_iter().take(limit as usize).collect();
    let organizations = convert_org_list(truncated)?;
    let next_cursor = if has_more {
        organizations.last().map(|last| last.id.to_string())
    } else {
        None
    };
    Ok(ListOrganizationsResponse {
        organizations,
        next_cursor,
    })
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
    let Some(permission) = operation.required_permission() else {
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
    let has = store
        .has_organization_permission(org_id, account_id, permission)
        .await?;
    if tanren_identity_policy::would_violate_last_admin(count, has) {
        return Err(AppServiceError::Organization(
            OrganizationFailureReason::LastAdminHolder,
        ));
    }
    Ok(())
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
