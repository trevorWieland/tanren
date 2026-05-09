//! Read-only self-permission query handler.
//!
//! This handler performs no side effects. It only reads the
//! authenticated account's effective permissions from the store.

use tanren_contract::{
    MyAccountCapabilitiesResponse, MyOrganizationPermissions, MyPermissionEntry,
    MyPermissionsFailureReason, MyPermissionsFreshnessMeta, MyPermissionsPageMeta,
    MyPermissionsRequest, MyPermissionsResponse, MyPermissionsStaleness, MyProjectPermissions,
    PermissionConstraintView,
};
use tanren_store::{AccountStore, MyPermissionsCursor, MyPermissionsPage, MyPermissionsRecord};

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
    let request_cursor = request.resolved_cursor();
    let decoded_cursor = decode_cursor(request_cursor.as_deref())?;

    let record = store
        .my_permissions(
            context.session_account_id(),
            MyPermissionsPage::bounded(Some(resolved_limit), decoded_cursor),
        )
        .await?;
    Ok(to_contract_response(record, resolved_limit, request_cursor))
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

fn to_contract_response(
    record: MyPermissionsRecord,
    resolved_limit: u16,
    request_cursor: Option<String>,
) -> MyPermissionsResponse {
    let returned_entries = total_entries(&record);
    let next_cursor = encode_cursor(record.next_cursor);
    let staleness = if record.freshness.is_stale {
        MyPermissionsStaleness::Stale
    } else {
        MyPermissionsStaleness::Fresh
    };
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
            request_cursor,
            next_cursor,
        },
        freshness: MyPermissionsFreshnessMeta {
            projection: record.freshness.projection,
            checkpoint: record.freshness.checkpoint,
            generated_at: record.freshness.generated_at,
            staleness,
        },
        organizations,
        projects,
    }
}

fn decode_cursor(token: Option<&str>) -> Result<Option<MyPermissionsCursor>, AppServiceError> {
    let Some(token) = token else {
        return Ok(None);
    };
    let cursor = serde_json::from_str(token)
        .map_err(|_| AppServiceError::InvalidInput("cursor must be valid JSON".to_owned()))?;
    Ok(Some(cursor))
}

fn encode_cursor(cursor: Option<MyPermissionsCursor>) -> Option<String> {
    cursor.and_then(|value| serde_json::to_string(&value).ok())
}

fn to_permission_entry(record: tanren_store::MyPermissionRecord) -> MyPermissionEntry {
    let grant_source_reference = grant_source_reference(&record);
    let policy_constraint = record
        .policy_constraint
        .map(|constraint| PermissionConstraintView {
            reason: constraint.reason,
            source: constraint.source,
            source_reference: format!("permission_constraint:{}", constraint.id.as_uuid()),
        });

    MyPermissionEntry {
        permission: record.permission,
        effective_state: record.effective_state,
        grant_source: record.grant_source,
        grant_source_reference,
        policy_constraint,
    }
}

fn grant_source_reference(record: &tanren_store::MyPermissionRecord) -> String {
    let grant_id = record.grant_id.as_uuid();
    match &record.grant_source {
        tanren_identity_policy::PermissionGrantSource::Direct => {
            format!("permission_grant:{grant_id}")
        }
        tanren_identity_policy::PermissionGrantSource::RoleTemplate { role_template } => {
            format!(
                "permission_grant:{grant_id}#role_template={}",
                role_template.as_str()
            )
        }
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
