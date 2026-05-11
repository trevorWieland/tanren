//! Internal helpers for organization-secret handlers.

use chrono::{DateTime, Utc};
use tanren_contract::{
    LIST_ORG_SECRETS_DEFAULT_LIMIT, LIST_ORG_SECRETS_MAX_LIMIT, OrganizationSecretView,
    ValueRedacted,
};
use tanren_identity_policy::{
    AccountId, OrgId, OrganizationPermission, OrganizationPermissionDecision,
    OrganizationPermissionGate, SessionToken, evaluate_organization_permission_gate,
};
use tanren_store::{AccountStore, OrganizationSecretRecord, SessionRecord};

use crate::OrganizationSecretServiceError;

/// Resolve and validate the session token for the requesting account.
pub(super) async fn resolve_authenticated_account<S>(
    store: &S,
    account_id: AccountId,
    session_token: &SessionToken,
    now: DateTime<Utc>,
) -> Result<SessionRecord, OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
{
    let session = store.find_session_by_token(session_token).await?;
    let Some(session) = session else {
        return Err(OrganizationSecretServiceError::AuthRequired);
    };
    if session.account_id != account_id || session.expires_at <= now {
        return Err(OrganizationSecretServiceError::AuthRequired);
    }
    Ok(session)
}

/// Enforce that the account holds the `Configure` permission for the org.
pub(super) async fn enforce_configure_permission<S>(
    store: &S,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<(), OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
{
    let gate = OrganizationPermissionGate::from_permission(OrganizationPermission::Configure);
    let allowed = store
        .has_organization_permission(account_id, org_id, gate.permission)
        .await
        .map_err(OrganizationSecretServiceError::Store)?;
    if matches!(
        evaluate_organization_permission_gate(allowed),
        OrganizationPermissionDecision::Allow
    ) {
        return Ok(());
    }
    Err(OrganizationSecretServiceError::PermissionDenied)
}

/// Enforce that the account is a member of the organization.
///
/// Checks multiple permissions as a membership proxy since the store
/// exposes `has_organization_permission` rather than a dedicated
/// membership check.
pub(super) async fn enforce_membership<S>(
    store: &S,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<(), OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
{
    for permission in &[
        OrganizationPermission::Configure,
        OrganizationPermission::Invite,
        OrganizationPermission::ManageAccess,
        OrganizationPermission::SetPolicy,
        OrganizationPermission::Delete,
    ] {
        let has = store
            .has_organization_permission(account_id, org_id, *permission)
            .await
            .map_err(OrganizationSecretServiceError::Store)?;
        if has {
            return Ok(());
        }
    }
    Err(OrganizationSecretServiceError::AuthRequired)
}

/// Project a store record into a metadata-only view with value redaction.
pub(super) fn project_secret_view(record: &OrganizationSecretRecord) -> OrganizationSecretView {
    OrganizationSecretView {
        id: record.id,
        org_id: record.org_id,
        name: record.name.clone(),
        owner_scope: record.owner_scope,
        status: record.status,
        use_policy: record.use_policy,
        version: record.version,
        description: record.description.clone(),
        provider: record.provider.clone(),
        value_redacted: ValueRedacted,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

/// Append an organization-secret event to the canonical event log.
pub(super) async fn append_secret_event<S, T>(
    store: &S,
    kind: &str,
    payload: &T,
    now: DateTime<Utc>,
) -> Result<(String, DateTime<Utc>), OrganizationSecretServiceError>
where
    S: AccountStore + ?Sized,
    T: serde::Serialize,
{
    let envelope = crate::events::organization_secret_envelope(kind, payload);
    let event = store
        .append_event(envelope, now)
        .await
        .map_err(OrganizationSecretServiceError::Store)?;
    Ok((event.id.to_string(), event.occurred_at))
}

pub(super) fn normalize_list_limit(limit: Option<u64>) -> u64 {
    let requested = limit.unwrap_or(LIST_ORG_SECRETS_DEFAULT_LIMIT);
    requested.clamp(1, LIST_ORG_SECRETS_MAX_LIMIT)
}
