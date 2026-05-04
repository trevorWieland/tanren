//! Organization-flow handlers: create, list, membership-permissions readback.
//!
//! Handlers resolve the caller's session to an account, then delegate
//! persistence to the store's atomic operations. Session resolution is
//! a private helper scoped to this module.

use chrono::{DateTime, Utc};
use tanren_contract::{
    CreateOrganizationRequest, CreateOrganizationResponse, ListOrganizationsResponse,
    OrganizationFailureReason, OrganizationView,
};
use tanren_identity_policy::{
    AccountId, MembershipId, OrgAdminPermissions, OrgId, OrgName, SessionToken, ValidationError,
};
use tanren_store::{
    AccountStore, CreateOrganizationAtomicRequest, CreateOrganizationError,
    CreateOrganizationEventContext, OrganizationRecord, StoreError,
};

use crate::events::{OrganizationCreated, OrganizationEventKind, organization_envelope};
use crate::{AppServiceError, Clock};

pub(crate) async fn create_organization<S>(
    store: &S,
    clock: &Clock,
    session: &SessionToken,
    request: CreateOrganizationRequest,
) -> Result<CreateOrganizationResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let account_id = resolve_session(store, session, now).await?;

    let permissions = OrgAdminPermissions::bootstrap_creator();
    let org_id = OrgId::fresh();
    let membership_id = MembershipId::fresh();
    let name = request.name.as_str().to_owned();
    let name_normalized = request.name.as_str().to_lowercase();

    let events_builder = build_organization_created_events_builder(permissions, now);

    let outcome = store
        .create_organization_atomic(CreateOrganizationAtomicRequest {
            org_id,
            name,
            name_normalized,
            now,
            creator_account_id: account_id,
            membership_id,
            permissions: permissions.to_bits(),
            events_builder,
        })
        .await
        .map_err(map_create_organization_error)?;

    let view = try_organization_view(&outcome.organization)?;
    Ok(CreateOrganizationResponse {
        organization: view,
        membership_permissions: permissions,
    })
}

pub(crate) async fn list_organizations<S>(
    store: &S,
    clock: &Clock,
    session: &SessionToken,
) -> Result<ListOrganizationsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let account_id = resolve_session(store, session, now).await?;

    let orgs = store.list_organizations_for_account(account_id).await?;
    let views: Vec<OrganizationView> = orgs
        .iter()
        .map(try_organization_view)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ListOrganizationsResponse {
        organizations: views,
    })
}

pub(crate) async fn membership_permissions<S>(
    store: &S,
    account_id: AccountId,
    org_id: OrgId,
) -> Result<OrgAdminPermissions, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let membership = store.find_membership(account_id, org_id).await?;
    match membership {
        Some(m) => Ok(OrgAdminPermissions::from_bits(m.permissions)),
        None => Ok(OrgAdminPermissions {
            invite: false,
            manage_access: false,
            configure: false,
            set_policy: false,
            delete: false,
        }),
    }
}

async fn resolve_session<S>(
    store: &S,
    token: &SessionToken,
    now: DateTime<Utc>,
) -> Result<AccountId, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let session = store
        .find_session_by_token(token)
        .await?
        .ok_or_else(|| AppServiceError::Organization(OrganizationFailureReason::Unauthenticated))?;
    if session.expires_at <= now {
        return Err(AppServiceError::Organization(
            OrganizationFailureReason::Unauthenticated,
        ));
    }
    Ok(session.account_id)
}

fn build_organization_created_events_builder(
    permissions: OrgAdminPermissions,
    now: DateTime<Utc>,
) -> tanren_store::CreateOrganizationEventsBuilder {
    Box::new(
        move |ctx: &CreateOrganizationEventContext| -> Vec<serde_json::Value> {
            vec![organization_envelope(
                OrganizationEventKind::OrganizationCreated,
                &OrganizationCreated {
                    org_id: ctx.org_id,
                    creator_account_id: ctx.creator_account_id,
                    permissions,
                    at: now,
                },
            )]
        },
    )
}

fn map_create_organization_error(err: CreateOrganizationError) -> AppServiceError {
    match err {
        CreateOrganizationError::DuplicateName => {
            AppServiceError::Organization(OrganizationFailureReason::DuplicateName)
        }
        CreateOrganizationError::Store(store_err) => AppServiceError::Store(store_err),
    }
}

fn try_organization_view(record: &OrganizationRecord) -> Result<OrganizationView, AppServiceError> {
    let name = OrgName::parse(&record.name).map_err(|ie| {
        let ve = match ie {
            tanren_identity_policy::IdentityError::Validation(v) => v,
            _ => ValidationError::EmptyOrgName,
        };
        AppServiceError::Store(StoreError::DataInvariant {
            column: "organization_name",
            cause: ve,
        })
    })?;
    Ok(OrganizationView {
        id: record.id,
        name,
        created_at: record.created_at,
    })
}
