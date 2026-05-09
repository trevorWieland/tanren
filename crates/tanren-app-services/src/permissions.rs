//! Read-only self-permission query handler.
//!
//! This handler performs no side effects. It only reads the
//! authenticated account's effective permissions from the store.

use tanren_contract::{
    MyAccountCapabilitiesResponse, MyOrganizationPermissions, MyPermissionEntry,
    MyPermissionsFailureReason, MyPermissionsPageMeta, MyPermissionsRequest, MyPermissionsResponse,
    MyProjectPermissions, PermissionConstraintView,
};
use tanren_store::{AccountStore, MyPermissionsPage, MyPermissionsRecord};

use crate::{AppServiceError, MyPermissionsContext};

pub(crate) async fn my_permissions<S>(
    store: &S,
    context: MyPermissionsContext,
    request: MyPermissionsRequest,
) -> Result<MyPermissionsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    authorize_my_permissions(context)?;
    let resolved_limit = request.resolved_limit();

    let record = store
        .my_permissions(
            context.session_account_id(),
            MyPermissionsPage::bounded(Some(resolved_limit)),
        )
        .await?;
    Ok(to_contract_response(record, resolved_limit))
}

pub(crate) fn my_permissions_capabilities(
    context: MyPermissionsContext,
) -> Result<MyAccountCapabilitiesResponse, AppServiceError> {
    authorize_my_permissions(context)?;
    Ok(MyAccountCapabilitiesResponse {
        can_view_my_permissions: true,
    })
}

fn authorize_my_permissions(context: MyPermissionsContext) -> Result<(), AppServiceError> {
    if context.requested_account_id() == context.session_account_id() {
        return Ok(());
    }
    Err(AppServiceError::Permissions(
        MyPermissionsFailureReason::PermissionDenied,
    ))
}

fn to_contract_response(record: MyPermissionsRecord, resolved_limit: u16) -> MyPermissionsResponse {
    let returned_entries = total_entries(&record);
    let organizations = record
        .organizations
        .into_iter()
        .map(|section| MyOrganizationPermissions {
            org_id: section.org_id,
            permissions: section
                .permissions
                .into_iter()
                .map(to_permission_entry)
                .collect(),
        })
        .collect();
    let projects = record
        .projects
        .into_iter()
        .map(|section| MyProjectPermissions {
            project_id: section.project_id,
            permissions: section
                .permissions
                .into_iter()
                .map(to_permission_entry)
                .collect(),
        })
        .collect();

    MyPermissionsResponse {
        page: MyPermissionsPageMeta {
            limit: resolved_limit,
            returned: u16::try_from(returned_entries).unwrap_or(u16::MAX),
        },
        organizations,
        projects,
    }
}

fn to_permission_entry(record: tanren_store::MyPermissionRecord) -> MyPermissionEntry {
    let policy_constraint = record
        .policy_constraint
        .map(|constraint| PermissionConstraintView {
            reason: constraint.reason,
            source: constraint.source,
        });

    MyPermissionEntry {
        permission: record.permission,
        effective_state: record.effective_state,
        grant_source: record.grant_source,
        policy_constraint,
    }
}

fn total_entries(record: &MyPermissionsRecord) -> usize {
    let organization_entries: usize = record
        .organizations
        .iter()
        .map(|section| section.permissions.len())
        .sum();
    let project_entries: usize = record
        .projects
        .iter()
        .map(|section| section.permissions.len())
        .sum();
    organization_entries + project_entries
}
