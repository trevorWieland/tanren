//! Organization-secret lifecycle handlers: create, update, delete, list,
//! read, and use.
//!
//! Admin-only management flows (create, update, delete) enforce the
//! organization `Configure` permission before side effects. Baseline use
//! policy governs the `use` operation: `MemberUse` permits any org member
//! to use the secret through a resolve-then-forget path; `AdminOnly`
//! rejects non-admin use. Read, detail, and list projections return
//! metadata-only views with explicit redaction markers — secret values
//! never leave this layer.
//!
//! Events record actor, org, secret metadata, version/status, and
//! operation kind while omitting raw values and encrypted payloads.
//! Event append is a mandatory part of accepting each command — not
//! best-effort decoration — so the canonical event log stays consistent
//! with store mutations.

mod helpers;

use tanren_configuration_secrets::SecretLifecycleStatus;
use tanren_contract::{
    CreateOrganizationSecretRequest, CreateOrganizationSecretResponse,
    DeleteOrganizationSecretRequest, DeleteOrganizationSecretResponse,
    ListOrganizationSecretsRequest, ListOrganizationSecretsResponse, ORG_SECRET_CREATED_EVENT_KIND,
    ORG_SECRET_DELETED_EVENT_KIND, ORG_SECRET_EVENT_FAMILY, ORG_SECRET_UPDATED_EVENT_KIND,
    ORG_SECRET_USED_EVENT_KIND, OrganizationSecretCreatedEvent, OrganizationSecretDeletedEvent,
    OrganizationSecretEventReference, OrganizationSecretProofLink,
    OrganizationSecretReadModelFreshness, OrganizationSecretSourceLink,
    OrganizationSecretSummaryView, OrganizationSecretUpdatedEvent, OrganizationSecretUsedEvent,
    ReadOrganizationSecretRequest, ReadOrganizationSecretResponse, UpdateOrganizationSecretRequest,
    UpdateOrganizationSecretResponse, UseOrganizationSecretRequest, UseOrganizationSecretResponse,
    ValueRedacted,
};
use tanren_observation::{ClaimValueKind, CompletenessState, FreshnessState, VisibilityState};
use tanren_store::{
    AccountStore, CreateOrganizationSecretError, CreateOrganizationSecretInput,
    RemoveOrganizationSecretError, RemoveOrganizationSecretInput, ResolveOrganizationSecretError,
    UpdateOrganizationSecretError, UpdateOrganizationSecretInput,
};

use crate::{Clock, OrganizationSecretServiceError};
use helpers::{
    append_secret_event, enforce_configure_permission, enforce_membership, normalize_list_limit,
    project_secret_view, resolve_authenticated_account,
};

/// Create an organization secret. Admin-only: requires `Configure`.
pub async fn create_organization_secret<S>(
    store: &S,
    clock: &Clock,
    request: CreateOrganizationSecretRequest,
) -> Result<CreateOrganizationSecretResponse, OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, request.account_id, &request.session_token, now).await?;
    enforce_configure_permission(store, request.account_id, request.org_id).await?;

    let body = request.body;
    let input = CreateOrganizationSecretInput {
        org_id: request.org_id,
        name: body.name.clone(),
        secret_value: body.secret_value,
        owner_scope: tanren_configuration_secrets::SecretOwnerScope::Organization,
        use_policy: body.use_policy,
        description: body.description,
        provider: body.provider,
        creator_account_id: request.account_id,
        now,
    };

    let output = store
        .create_organization_secret(input)
        .await
        .map_err(|err| match err {
            CreateOrganizationSecretError::DuplicateName => {
                OrganizationSecretServiceError::Conflict
            }
            CreateOrganizationSecretError::Store(err) => OrganizationSecretServiceError::Store(err),
        })?;

    let event_payload = OrganizationSecretCreatedEvent {
        secret_id: output.record.id,
        org_id: output.record.org_id,
        name: output.record.name.clone(),
        owner_scope: output.record.owner_scope,
        use_policy: output.record.use_policy,
        version: output.record.version,
        creator_account_id: request.account_id,
        created_at: now,
    };

    let (event_id, event_at) =
        append_secret_event(store, ORG_SECRET_CREATED_EVENT_KIND, &event_payload, now).await?;

    Ok(CreateOrganizationSecretResponse {
        secret: project_secret_view(&output.record),
        proof_link: OrganizationSecretProofLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_CREATED_EVENT_KIND.to_owned(),
        },
        source_link: OrganizationSecretSourceLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_CREATED_EVENT_KIND.to_owned(),
        },
        source_event: Some(OrganizationSecretEventReference {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_CREATED_EVENT_KIND.to_owned(),
            event_id,
            cursor: String::new(),
            occurred_at: event_at,
        }),
    })
}

/// Update an organization secret value. Admin-only: requires `Configure`.
pub async fn update_organization_secret<S>(
    store: &S,
    clock: &Clock,
    request: UpdateOrganizationSecretRequest,
) -> Result<UpdateOrganizationSecretResponse, OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, request.account_id, &request.session_token, now).await?;
    enforce_configure_permission(store, request.account_id, request.org_id).await?;

    let body = request.body;
    let input = UpdateOrganizationSecretInput {
        org_id: request.org_id,
        secret_id: request.secret_id,
        secret_value: body.secret_value,
        description: body.description.map(Some),
        status: None,
        use_policy: body.use_policy,
        updater_account_id: request.account_id,
        now,
    };

    let output = store
        .update_organization_secret(input)
        .await
        .map_err(|err| match err {
            UpdateOrganizationSecretError::NotFound => OrganizationSecretServiceError::NotFound,
            UpdateOrganizationSecretError::Store(err) => OrganizationSecretServiceError::Store(err),
        })?;

    let event_payload = OrganizationSecretUpdatedEvent {
        secret_id: output.record.id,
        org_id: output.record.org_id,
        name: output.record.name.clone(),
        version: output.record.version,
        updater_account_id: request.account_id,
        updated_at: now,
    };

    let (event_id, event_at) =
        append_secret_event(store, ORG_SECRET_UPDATED_EVENT_KIND, &event_payload, now).await?;

    Ok(UpdateOrganizationSecretResponse {
        secret: project_secret_view(&output.record),
        proof_link: OrganizationSecretProofLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_UPDATED_EVENT_KIND.to_owned(),
        },
        source_link: OrganizationSecretSourceLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_UPDATED_EVENT_KIND.to_owned(),
        },
        source_event: Some(OrganizationSecretEventReference {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_UPDATED_EVENT_KIND.to_owned(),
            event_id,
            cursor: String::new(),
            occurred_at: event_at,
        }),
    })
}

/// Soft-delete an organization secret. Admin-only: requires `Configure`.
pub async fn delete_organization_secret<S>(
    store: &S,
    clock: &Clock,
    request: DeleteOrganizationSecretRequest,
) -> Result<DeleteOrganizationSecretResponse, OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, request.account_id, &request.session_token, now).await?;
    enforce_configure_permission(store, request.account_id, request.org_id).await?;

    let input = RemoveOrganizationSecretInput {
        org_id: request.org_id,
        secret_id: request.secret_id,
        remover_account_id: request.account_id,
        now,
    };

    let output = store
        .remove_organization_secret(input)
        .await
        .map_err(|err| match err {
            RemoveOrganizationSecretError::NotFound => OrganizationSecretServiceError::NotFound,
            RemoveOrganizationSecretError::Store(err) => OrganizationSecretServiceError::Store(err),
        })?;

    let event_payload = OrganizationSecretDeletedEvent {
        secret_id: output.record.id,
        org_id: output.record.org_id,
        name: output.record.name.clone(),
        deleter_account_id: request.account_id,
        deleted_at: now,
    };

    let (event_id, event_at) =
        append_secret_event(store, ORG_SECRET_DELETED_EVENT_KIND, &event_payload, now).await?;

    Ok(DeleteOrganizationSecretResponse {
        secret: project_secret_view(&output.record),
        proof_link: OrganizationSecretProofLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_DELETED_EVENT_KIND.to_owned(),
        },
        source_link: OrganizationSecretSourceLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_DELETED_EVENT_KIND.to_owned(),
        },
        source_event: Some(OrganizationSecretEventReference {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_DELETED_EVENT_KIND.to_owned(),
            event_id,
            cursor: String::new(),
            occurred_at: event_at,
        }),
    })
}

/// Read a single organization secret. Membership-only; metadata projection.
pub async fn read_organization_secret<S>(
    store: &S,
    clock: &Clock,
    request: ReadOrganizationSecretRequest,
) -> Result<ReadOrganizationSecretResponse, OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, request.account_id, &request.session_token, now).await?;
    enforce_membership(store, request.account_id, request.org_id).await?;

    let record = store
        .get_organization_secret(request.org_id, request.secret_id)
        .await
        .map_err(OrganizationSecretServiceError::Store)?
        .ok_or(OrganizationSecretServiceError::NotFound)?;

    Ok(ReadOrganizationSecretResponse {
        secret: project_secret_view(&record),
        value_redacted: ValueRedacted,
        proof_link: OrganizationSecretProofLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_CREATED_EVENT_KIND.to_owned(),
        },
        source_link: OrganizationSecretSourceLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_CREATED_EVENT_KIND.to_owned(),
        },
        freshness: OrganizationSecretReadModelFreshness {
            visibility: VisibilityState::Visible,
            completeness: CompletenessState::Complete,
            freshness: FreshnessState::Fresh,
            claim_value_kind: ClaimValueKind::Measured,
        },
    })
}

/// List organization secrets. Membership-only; metadata summary views.
pub async fn list_organization_secrets<S>(
    store: &S,
    clock: &Clock,
    request: ListOrganizationSecretsRequest,
) -> Result<ListOrganizationSecretsResponse, OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, request.account_id, &request.session_token, now).await?;
    enforce_membership(store, request.account_id, request.org_id).await?;

    let limit = normalize_list_limit(request.limit);
    let page = store
        .list_organization_secrets(tanren_store::ListOrganizationSecretsRequest {
            org_id: request.org_id,
            limit,
            cursor: None,
        })
        .await
        .map_err(OrganizationSecretServiceError::Store)?;

    let secrets: Vec<OrganizationSecretSummaryView> = page
        .records
        .into_iter()
        .filter(|r| r.status != SecretLifecycleStatus::Retired)
        .map(|record| OrganizationSecretSummaryView {
            id: record.id,
            name: record.name,
            status: record.status,
            use_policy: record.use_policy,
            version: record.version,
            provider: record.provider,
            value_redacted: ValueRedacted,
        })
        .collect();

    Ok(ListOrganizationSecretsResponse {
        secrets,
        next_cursor: page.next_cursor.map(|id| id.to_string()),
        source_link: OrganizationSecretSourceLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_CREATED_EVENT_KIND.to_owned(),
        },
        freshness: OrganizationSecretReadModelFreshness {
            visibility: VisibilityState::Visible,
            completeness: CompletenessState::Complete,
            freshness: FreshnessState::Fresh,
            claim_value_kind: ClaimValueKind::Measured,
        },
    })
}

/// Use (resolve) a secret. Value consumed in-memory; never returned.
///
/// Policy is evaluated BEFORE the secret value is resolved so the
/// governed sensitive operation (decryption) only happens for actors
/// who pass the baseline-use-policy check. The event is appended as
/// a mandatory part of accepting the command — not as best-effort
/// decoration — so the canonical event log stays consistent with
/// the store mutation.
pub async fn use_organization_secret<S>(
    store: &S,
    clock: &Clock,
    request: UseOrganizationSecretRequest,
) -> Result<UseOrganizationSecretResponse, OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    resolve_authenticated_account(store, request.account_id, &request.session_token, now).await?;

    // Policy check BEFORE value resolution: read metadata only to
    // decide whether the actor is permitted, then resolve the value
    // only after the policy gate passes.
    let record = store
        .get_organization_secret(request.org_id, request.secret_id)
        .await
        .map_err(OrganizationSecretServiceError::Store)?
        .ok_or(OrganizationSecretServiceError::NotFound)?;

    match record.use_policy {
        tanren_configuration_secrets::BaselineUsePolicy::MemberUse => {
            enforce_membership(store, request.account_id, request.org_id).await?;
        }
        tanren_configuration_secrets::BaselineUsePolicy::AdminOnly => {
            enforce_configure_permission(store, request.account_id, request.org_id).await?;
        }
        _ => {
            // Non-exhaustive: future variants require admin by default.
            enforce_configure_permission(store, request.account_id, request.org_id).await?;
        }
    }

    // Policy passed — now safe to resolve (decrypt) the value.
    let output = store
        .resolve_organization_secret(request.org_id, request.secret_id)
        .await
        .map_err(|err| match err {
            ResolveOrganizationSecretError::NotFound => OrganizationSecretServiceError::NotFound,
            ResolveOrganizationSecretError::UseDenied => {
                OrganizationSecretServiceError::PermissionDenied
            }
            ResolveOrganizationSecretError::Store(err) => {
                OrganizationSecretServiceError::Store(err)
            }
        })?;

    drop(output.value);

    let event_payload = OrganizationSecretUsedEvent {
        secret_id: output.record.id,
        org_id: output.record.org_id,
        name: output.record.name.clone(),
        version: output.record.version,
        actor_account_id: request.account_id,
        used_at: now,
    };

    let (event_id, event_at) =
        append_secret_event(store, ORG_SECRET_USED_EVENT_KIND, &event_payload, now).await?;

    Ok(UseOrganizationSecretResponse {
        secret: project_secret_view(&output.record),
        value_redacted: ValueRedacted,
        proof_link: OrganizationSecretProofLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_USED_EVENT_KIND.to_owned(),
        },
        source_link: OrganizationSecretSourceLink {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_USED_EVENT_KIND.to_owned(),
        },
        source_event: Some(OrganizationSecretEventReference {
            event_family: ORG_SECRET_EVENT_FAMILY.to_owned(),
            event_kind: ORG_SECRET_USED_EVENT_KIND.to_owned(),
            event_id,
            cursor: String::new(),
            occurred_at: event_at,
        }),
    })
}
