//! Read-only self-permission query handler.
//!
//! This handler performs no side effects. It only reads the
//! authenticated account's effective permissions from the store.

use tanren_contract::{
    MyOrganizationPermissions, MyPermissionEntry, MyPermissionsRequest, MyPermissionsResponse,
    MyProjectPermissions, PermissionConstraintView,
};
use tanren_store::{AccountStore, MyPermissionsPage, MyPermissionsRecord};

use crate::{AppServiceError, MyPermissionsContext, PermissionsFailureReason};

pub(crate) async fn my_permissions<S>(
    store: &S,
    context: MyPermissionsContext,
    request: MyPermissionsRequest,
) -> Result<MyPermissionsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    if context.requested_account_id != context.session_account_id {
        return Err(AppServiceError::Permissions(
            PermissionsFailureReason::PermissionDenied,
        ));
    }

    let record = store
        .my_permissions(
            context.session_account_id,
            MyPermissionsPage::bounded(Some(request.resolved_limit())),
        )
        .await?;
    Ok(to_contract_response(record))
}

fn to_contract_response(record: MyPermissionsRecord) -> MyPermissionsResponse {
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
