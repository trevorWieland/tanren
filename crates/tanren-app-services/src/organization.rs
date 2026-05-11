//! Organization-flow handlers.

use chrono::{DateTime, Utc};
use tanren_contract::{
    AccountFailureReason, CheckOrganizationPermissionRequest, CheckOrganizationPermissionResponse,
    CreateOrganizationFailureReason, CreateOrganizationRequest, CreateOrganizationResponse,
    GrantSource, LIST_ORGANIZATION_MEMBERS_DEFAULT_LIMIT, LIST_ORGANIZATION_MEMBERS_MAX_LIMIT,
    LIST_ORGANIZATIONS_DEFAULT_LIMIT, LIST_ORGANIZATIONS_MAX_LIMIT, ListOrganizationMembersRequest,
    ListOrganizationMembersResponse, ListOrganizationsRequest, ListOrganizationsResponse,
    ORGANIZATION_CREATED_EVENT_KIND, ORGANIZATION_EVENT_FAMILY, OrganizationBehaviorId,
    OrganizationCreatedEvent, OrganizationEventReference, OrganizationMemberPermissionGrant,
    OrganizationMemberView, OrganizationProjectSummary, OrganizationProofLink,
    OrganizationSourceLink, OrganizationView, ReadModelFreshness,
    organization_capability_projection, organization_permission_options,
};
use tanren_identity_policy::{
    AccountId, OrgId, OrganizationPermission, OrganizationPermissionDecision,
    OrganizationPermissionGate, SessionToken, evaluate_organization_permission_gate,
};
use tanren_observation::{ClaimValueKind, CompletenessState, FreshnessState, VisibilityState};
use tanren_store::{
    AccountStore, CreateOrganizationAtomicRequest, CreateOrganizationError, SessionRecord,
};
use tanren_store::{GrantSource as StoreGrantSource, OrganizationPermissionGrantRecord};

use crate::{AppServiceError, Clock, events::organization_created_envelope};

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
            capabilities: organization_capability_projection(output.granted_permissions.clone()),
        },
        capabilities: organization_capability_projection(output.granted_permissions.clone()),
        available_permissions: organization_permission_options(),
        granted_permissions: output.granted_permissions,
        initial_project_count: output.initial_project_count,
        project_summary: OrganizationProjectSummary {
            total_count: output.initial_project_count,
        },
        proof_link: OrganizationProofLink {
            behavior_id: OrganizationBehaviorId::B0066CreateOrganization,
        },
        source_link: OrganizationSourceLink {
            event_family: ORGANIZATION_EVENT_FAMILY.to_owned(),
            event_kind: ORGANIZATION_CREATED_EVENT_KIND.to_owned(),
        },
        source_event: output
            .source_event
            .map(|source_event| OrganizationEventReference {
                event_family: ORGANIZATION_EVENT_FAMILY.to_owned(),
                event_kind: ORGANIZATION_CREATED_EVENT_KIND.to_owned(),
                event_id: source_event.id.clone(),
                cursor: source_event.id,
                occurred_at: source_event.occurred_at,
            }),
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
        .list_organizations_for_account(request.account_id, limit, request.cursor, now)
        .await?;
    let organizations = page
        .organizations
        .into_iter()
        .map(|record| OrganizationView {
            id: record.organization.id,
            name: record.organization.name,
            capabilities: organization_capability_projection(record.granted_permissions),
        })
        .collect();
    Ok(ListOrganizationsResponse {
        organizations,
        next_cursor: page.next_cursor,
        source_link: OrganizationSourceLink {
            event_family: ORGANIZATION_EVENT_FAMILY.to_owned(),
            event_kind: ORGANIZATION_CREATED_EVENT_KIND.to_owned(),
        },
        freshness: ReadModelFreshness {
            projection: "organizations_by_account_membership".to_owned(),
            checkpoint: page.checkpoint,
            generated_at: page.generated_at,
            cursor: page.cursor,
            source: "organization_membership_store".to_owned(),
            value_kind: ClaimValueKind::Measured,
            completeness: CompletenessState::Complete,
            freshness_state: FreshnessState::Fresh,
            visibility: VisibilityState::Visible,
        },
    })
}

pub(crate) async fn list_organization_members<S>(
    store: &S,
    clock: &Clock,
    request: ListOrganizationMembersRequest,
) -> Result<ListOrganizationMembersResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, request.account_id, &request.session_token, now).await?;

    let is_member = store
        .has_organization_membership(request.account_id, request.org_id)
        .await?;
    if !is_member {
        return Err(AppServiceError::Account(
            AccountFailureReason::PermissionDenied,
        ));
    }

    let limit = normalize_member_list_limit(request.limit);
    let page = store
        .list_organization_members(request.org_id, limit, request.cursor, now)
        .await?;

    let members = page.members.into_iter().map(project_member_view).collect();

    Ok(ListOrganizationMembersResponse {
        members,
        next_cursor: page.next_cursor,
        source_link: OrganizationSourceLink {
            event_family: ORGANIZATION_EVENT_FAMILY.to_owned(),
            event_kind: ORGANIZATION_CREATED_EVENT_KIND.to_owned(),
        },
        freshness: ReadModelFreshness {
            projection: "organization_members_by_org".to_owned(),
            checkpoint: page.checkpoint,
            generated_at: page.generated_at,
            cursor: page.cursor,
            source: "organization_membership_store".to_owned(),
            value_kind: ClaimValueKind::Measured,
            completeness: CompletenessState::Complete,
            freshness_state: FreshnessState::Fresh,
            visibility: VisibilityState::Visible,
        },
        proof_link: OrganizationProofLink {
            behavior_id: OrganizationBehaviorId::B0065ListOrganizationMembers,
        },
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
    let gate = require_checked_organization_permission(store, clock, &request).await?;
    Ok(CheckOrganizationPermissionResponse {
        account_id: request.account_id,
        org_id: request.org_id,
        permission: gate.permission,
        allowed: true,
    })
}

async fn require_checked_organization_permission<S>(
    store: &S,
    clock: &Clock,
    request: &CheckOrganizationPermissionRequest,
) -> Result<OrganizationPermissionGate, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    if request.permission == OrganizationPermission::Configure {
        return require_configure_organization_permission(
            store,
            clock,
            &request.session_token,
            request.account_id,
            request.org_id,
        )
        .await;
    }
    require_organization_permission(
        store,
        clock,
        &request.session_token,
        request.account_id,
        request.org_id,
        request.permission,
    )
    .await
}

pub(crate) async fn require_organization_permission<S>(
    store: &S,
    clock: &Clock,
    session_token: &SessionToken,
    account_id: AccountId,
    org_id: OrgId,
    permission: OrganizationPermission,
) -> Result<OrganizationPermissionGate, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    require_organization_permission_gate(
        store,
        clock,
        session_token,
        account_id,
        org_id,
        OrganizationPermissionGate::from_permission(permission),
    )
    .await
}

pub(crate) async fn require_configure_organization_permission<S>(
    store: &S,
    clock: &Clock,
    session_token: &SessionToken,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<OrganizationPermissionGate, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    require_organization_permission(
        store,
        clock,
        session_token,
        account_id,
        org_id,
        OrganizationPermission::Configure,
    )
    .await
}

pub(crate) async fn require_organization_permission_gate<S>(
    store: &S,
    clock: &Clock,
    session_token: &SessionToken,
    account_id: AccountId,
    org_id: OrgId,
    gate: OrganizationPermissionGate,
) -> Result<OrganizationPermissionGate, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, account_id, session_token, now).await?;
    let allowed = store
        .has_organization_permission(account_id, org_id, gate.permission)
        .await?;
    if matches!(
        evaluate_organization_permission_gate(allowed),
        OrganizationPermissionDecision::Allow
    ) {
        return Ok(gate);
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
        let payload = OrganizationCreatedEvent {
            org_id: ctx.organization.id,
            name: ctx.organization.name.clone(),
            creator_account_id: ctx.creator_account_id,
            granted_permissions: ctx.granted_permissions.clone(),
            initial_project_count,
            created_at: ctx.now,
        };
        vec![organization_created_envelope(&payload)]
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

fn normalize_member_list_limit(limit: Option<u64>) -> u64 {
    let requested = limit.unwrap_or(LIST_ORGANIZATION_MEMBERS_DEFAULT_LIMIT);
    requested.clamp(1, LIST_ORGANIZATION_MEMBERS_MAX_LIMIT)
}

fn project_member_view(record: tanren_store::OrganizationMemberRecord) -> OrganizationMemberView {
    OrganizationMemberView {
        account_id: record.account_id,
        identifier: record.identifier.to_string(),
        joined_at: record.joined_at,
        granted_permissions: record
            .grants
            .into_iter()
            .map(|g| project_permission_grant(&g))
            .collect(),
    }
}

fn project_permission_grant(
    record: &OrganizationPermissionGrantRecord,
) -> OrganizationMemberPermissionGrant {
    OrganizationMemberPermissionGrant {
        permission: record.permission,
        grant_source: map_store_grant_source(record.grant_source),
        granted_by_account_id: record.granted_by_account_id,
    }
}

fn map_store_grant_source(source: StoreGrantSource) -> GrantSource {
    match source {
        StoreGrantSource::Direct => GrantSource::Direct,
        StoreGrantSource::RoleTemplate => GrantSource::RoleTemplate,
    }
}

fn normalize_list_limit(limit: Option<u64>) -> u64 {
    let requested = limit.unwrap_or(LIST_ORGANIZATIONS_DEFAULT_LIMIT);
    requested.clamp(1, LIST_ORGANIZATIONS_MAX_LIMIT)
}
