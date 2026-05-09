//! Organization-flow handlers.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tanren_contract::{
    AccountFailureReason, CheckOrganizationPermissionRequest, CheckOrganizationPermissionResponse,
    CreateOrganizationRequest, CreateOrganizationResponse, ListOrganizationsRequest,
    ListOrganizationsResponse, OrganizationView,
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

    let name = tanren_identity_policy::OrganizationName::parse(request.name.as_str())
        .map_err(|err| AppServiceError::InvalidInput(err.to_string()))?;
    let idempotency_key = normalize_idempotency_key(request.idempotency_key)?;
    let output = store
        .create_organization_atomic(CreateOrganizationAtomicRequest {
            organization_id: OrgId::fresh(),
            name,
            creator_account_id: request.account_id,
            creator_membership_id: tanren_identity_policy::MembershipId::fresh(),
            now,
            idempotency_key,
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
    let organizations = store
        .list_organizations_for_account(request.account_id)
        .await?
        .into_iter()
        .map(|record| OrganizationView {
            id: record.id,
            name: record.name,
        })
        .collect();
    Ok(ListOrganizationsResponse { organizations })
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

#[derive(Debug, Clone, Serialize)]
struct OrganizationCreatedEvent {
    org_id: OrgId,
    name: String,
    creator_account_id: AccountId,
    granted_permissions: Vec<OrganizationPermission>,
    initial_project_count: u64,
    created_at: DateTime<Utc>,
}

fn build_create_organization_events_builder() -> tanren_store::CreateOrganizationEventsBuilder {
    Box::new(|ctx| {
        vec![serde_json::json!({
            "family": "organization",
            "kind": "organization_created",
            "payload": OrganizationCreatedEvent {
                org_id: ctx.organization.id,
                name: ctx.organization.name.as_str().to_owned(),
                creator_account_id: ctx.creator_account_id,
                granted_permissions: ctx.granted_permissions.clone(),
                initial_project_count: 0,
                created_at: ctx.now,
            },
        })]
    })
}

fn map_create_organization_error(err: CreateOrganizationError) -> AppServiceError {
    match err {
        CreateOrganizationError::DuplicateName => {
            AppServiceError::InvalidInput("organization name already exists".to_owned())
        }
        CreateOrganizationError::IdempotencyConflict => {
            AppServiceError::InvalidInput("idempotency_conflict".to_owned())
        }
        CreateOrganizationError::Store(err) => AppServiceError::Store(err),
    }
}

fn normalize_idempotency_key(raw: Option<String>) -> Result<Option<String>, AppServiceError> {
    match raw {
        None => Ok(None),
        Some(key) => {
            let trimmed = key.trim();
            if trimmed.is_empty() {
                return Err(AppServiceError::InvalidInput(
                    "idempotency key must not be empty".to_owned(),
                ));
            }
            Ok(Some(trimmed.to_owned()))
        }
    }
}
