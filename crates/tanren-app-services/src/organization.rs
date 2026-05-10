//! Organization-flow handlers.

use chrono::{DateTime, Utc};
use tanren_contract::{
    AccountFailureReason, CheckOrganizationPermissionRequest, CheckOrganizationPermissionResponse,
    CreateOrganizationFailureReason, CreateOrganizationRequest, CreateOrganizationResponse,
    LIST_ORGANIZATIONS_DEFAULT_LIMIT, LIST_ORGANIZATIONS_MAX_LIMIT, ListOrganizationsRequest,
    ListOrganizationsResponse, ORGANIZATION_CREATE_BEHAVIOR_ID, ORGANIZATION_CREATED_EVENT_KIND,
    ORGANIZATION_EVENT_FAMILY, OrganizationCreatedEvent, OrganizationProofLink,
    OrganizationSourceLink, OrganizationView,
};
use tanren_identity_policy::{AccountId, OrgId, OrganizationPermission, SessionToken};
use tanren_store::{
    AccountStore, CreateOrganizationAtomicRequest, CreateOrganizationError, SessionRecord,
};

use crate::{AppServiceError, Clock};

pub(crate) async fn create_organization<S>(
    store: &S,
    clock: &Clock,
    request: CreateOrganizationRequest,
) -> Result<CreateOrganizationResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, request.account_id, &request.session_token, now).await?;

    let output = store
        .create_organization_atomic(CreateOrganizationAtomicRequest {
            organization_id: OrgId::fresh(),
            name: request.name,
            creator_account_id: request.account_id,
            creator_membership_id: tanren_identity_policy::MembershipId::fresh(),
            now,
            idempotency_key: request.idempotency_key,
            events_builder: build_create_organization_events_builder(),
        })
        .await
        .map_err(map_create_organization_error)?;

    Ok(CreateOrganizationResponse {
        organization: OrganizationView {
            id: output.organization.id,
            name: output.organization.name,
        },
        granted_permissions: output.granted_permissions,
        initial_project_count: output.initial_project_count,
        proof_link: OrganizationProofLink {
            behavior_id: ORGANIZATION_CREATE_BEHAVIOR_ID.to_owned(),
        },
        source_link: OrganizationSourceLink {
            event_family: ORGANIZATION_EVENT_FAMILY.to_owned(),
            event_kind: ORGANIZATION_CREATED_EVENT_KIND.to_owned(),
        },
    })
}

pub(crate) async fn list_organizations<S>(
    store: &S,
    clock: &Clock,
    request: ListOrganizationsRequest,
) -> Result<ListOrganizationsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, request.account_id, &request.session_token, now).await?;
    let limit = normalize_list_limit(request.limit);
    let page = store
        .list_organizations_for_account(request.account_id, limit, request.cursor)
        .await?;
    let organizations = page
        .organizations
        .into_iter()
        .map(|record| OrganizationView {
            id: record.id,
            name: record.name,
        })
        .collect();
    Ok(ListOrganizationsResponse {
        organizations,
        next_cursor: page.next_cursor,
    })
}

pub(crate) async fn check_organization_permission<S>(
    store: &S,
    clock: &Clock,
    request: CheckOrganizationPermissionRequest,
) -> Result<CheckOrganizationPermissionResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    require_organization_permission(
        store,
        clock,
        &request.session_token,
        request.account_id,
        request.org_id,
        request.permission,
    )
    .await?;
    Ok(CheckOrganizationPermissionResponse {
        account_id: request.account_id,
        org_id: request.org_id,
        permission: request.permission,
        allowed: true,
    })
}

pub(crate) async fn require_organization_permission<S>(
    store: &S,
    clock: &Clock,
    session_token: &SessionToken,
    account_id: AccountId,
    org_id: OrgId,
    permission: OrganizationPermission,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, account_id, session_token, now).await?;
    if store
        .has_organization_permission(account_id, org_id, permission)
        .await?
    {
        return Ok(());
    }
    Err(AppServiceError::Account(
        AccountFailureReason::PermissionDenied,
    ))
}

async fn resolve_authenticated_account<S>(
    store: &S,
    account_id: AccountId,
    session_token: &SessionToken,
    now: DateTime<Utc>,
) -> Result<SessionRecord, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let session = store.find_session_by_token(session_token).await?;
    let Some(session) = session else {
        return Err(AppServiceError::Account(AccountFailureReason::AuthRequired));
    };

    if session.account_id != account_id || session.expires_at <= now {
        return Err(AppServiceError::Account(AccountFailureReason::AuthRequired));
    }
    Ok(session)
}

fn build_create_organization_events_builder() -> tanren_store::CreateOrganizationEventsBuilder {
    Box::new(|ctx| {
        let initial_project_count = 0;
        vec![serde_json::json!({
            "family": ORGANIZATION_EVENT_FAMILY,
            "kind": ORGANIZATION_CREATED_EVENT_KIND,
            "payload": OrganizationCreatedEvent {
                org_id: ctx.organization.id,
                name: ctx.organization.name.clone(),
                creator_account_id: ctx.creator_account_id,
                granted_permissions: ctx.granted_permissions.clone(),
                initial_project_count,
                created_at: ctx.now,
            },
        })]
    })
}

fn map_create_organization_error(err: CreateOrganizationError) -> AppServiceError {
    match err {
        CreateOrganizationError::DuplicateName => {
            AppServiceError::CreateOrganization(CreateOrganizationFailureReason::DuplicateName)
        }
        CreateOrganizationError::IdempotencyConflict => AppServiceError::CreateOrganization(
            CreateOrganizationFailureReason::IdempotencyConflict,
        ),
        CreateOrganizationError::Store(err) => AppServiceError::Store(err),
    }
}

fn normalize_list_limit(limit: Option<u64>) -> u64 {
    let requested = limit.unwrap_or(LIST_ORGANIZATIONS_DEFAULT_LIMIT);
    requested.clamp(1, LIST_ORGANIZATIONS_MAX_LIMIT)
}
