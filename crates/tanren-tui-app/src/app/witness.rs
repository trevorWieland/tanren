use tanren_contract::{
    CheckOrganizationPermissionResponse, CreateOrganizationResponse, ListOrganizationsResponse,
};
use tanren_identity_policy::{AccountId, OrgId};

pub(super) fn app_ready() {
    tracing::info!("tui_witness op=app kind=ready");
}

pub(super) fn sign_up_success(account_id: AccountId, has_token: bool) {
    tracing::info!(
        "tui_witness op=sign_up kind=success account_id={} session_token_present={}",
        account_id,
        has_token
    );
}

pub(super) fn sign_in_success(account_id: AccountId, has_token: bool) {
    tracing::info!(
        "tui_witness op=sign_in kind=success account_id={} session_token_present={}",
        account_id,
        has_token
    );
}

pub(super) fn accept_invitation_success(account_id: AccountId, joined_org: OrgId, has_token: bool) {
    tracing::info!(
        "tui_witness op=accept_invitation kind=success account_id={} joined_org={} session_token_present={}",
        account_id,
        joined_org,
        has_token
    );
}

pub(super) fn create_organization_success(response: &CreateOrganizationResponse) {
    let permissions = response
        .granted_permissions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    tracing::info!(
        "tui_witness op=create_organization kind=success organization_id={} granted_permissions={} initial_project_count={} proof_behavior_id={} source_event={}.{}",
        response.organization.id,
        permissions,
        response.initial_project_count,
        response.proof_link.behavior_id,
        response.source_link.event_family,
        response.source_link.event_kind
    );
}

pub(super) fn list_organizations_success(response: &ListOrganizationsResponse) {
    tracing::info!(
        "tui_witness op=list_organizations kind=success count={}",
        response.organizations.len()
    );
    for org in &response.organizations {
        tracing::info!(
            "tui_witness op=list_organizations kind=row organization_id={} name={:?}",
            org.id,
            org.name.as_str()
        );
    }
}

pub(super) fn check_permission_success(response: &CheckOrganizationPermissionResponse) {
    tracing::info!(
        "tui_witness op=check_organization_permission kind=success account_id={} org_id={} permission={}",
        response.account_id,
        response.org_id,
        response.permission
    );
}

pub(super) fn operation_error(operation: &str, message: &str) {
    tracing::info!(
        "tui_witness op={} kind=error code={}",
        operation,
        failure_code_from_message(message)
    );
}

fn failure_code_from_message(message: &str) -> &str {
    message
        .split_once(':')
        .map_or("internal_error", |(code, _)| code.trim())
}
