//! Organization-switching and project-listing handlers.
//!
//! These handlers manage the active-organization context for an account:
//! listing memberships, switching the active org, and listing projects
//! scoped to the active org. Personal accounts (zero org memberships)
//! gracefully receive empty results without exposing organization-scoped
//! actions.

use tanren_contract::{
    AccountFailureReason, AccountView, ListOrganizationProjectsResponse,
    OrganizationMembershipView, OrganizationSwitcher, ProjectView, SwitchActiveOrganizationRequest,
    SwitchActiveOrganizationResponse,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountRecord, AccountStore, SetActiveOrgError};

use crate::AppServiceError;

pub(crate) async fn list_organizations<S>(
    store: &S,
    account_id: AccountId,
) -> Result<OrganizationSwitcher, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let account = store
        .find_account_by_id(account_id)
        .await?
        .ok_or_else(|| AppServiceError::InvalidInput("account not found".to_owned()))?;
    let orgs = store.list_account_organizations(account_id).await?;
    let memberships: Vec<OrganizationMembershipView> = orgs
        .into_iter()
        .map(|org| OrganizationMembershipView {
            org_id: org.id,
            org_name: org.name,
        })
        .collect();
    Ok(OrganizationSwitcher {
        memberships,
        active_org: account.active_org_id,
    })
}

pub(crate) async fn switch_active_org<S>(
    store: &S,
    account_id: AccountId,
    request: SwitchActiveOrganizationRequest,
) -> Result<SwitchActiveOrganizationResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .set_active_org(account_id, request.org_id)
        .await
        .map_err(|err| match err {
            SetActiveOrgError::NotAMember => {
                AppServiceError::Account(AccountFailureReason::OrganizationNotMember)
            }
            SetActiveOrgError::Store(store_err) => AppServiceError::Store(store_err),
        })?;
    let account = store
        .find_account_by_id(account_id)
        .await?
        .ok_or_else(|| AppServiceError::InvalidInput("account not found".to_owned()))?;
    Ok(SwitchActiveOrganizationResponse {
        account: account_view(&account),
    })
}

pub(crate) async fn list_active_org_projects<S>(
    store: &S,
    account_id: AccountId,
) -> Result<ListOrganizationProjectsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let projects = store.list_projects_for_active_org(account_id).await?;
    let views: Vec<ProjectView> = projects
        .into_iter()
        .map(|p| ProjectView {
            id: p.id,
            name: p.name,
            org: p.org_id,
        })
        .collect();
    Ok(ListOrganizationProjectsResponse { projects: views })
}

fn account_view(record: &AccountRecord) -> AccountView {
    AccountView {
        id: record.id,
        identifier: record.identifier.clone(),
        display_name: record.display_name.clone(),
        org: record.org_id,
    }
}
