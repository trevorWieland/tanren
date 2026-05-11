use super::api::AcceptInvitationCookieResponse;
use tanren_contract::{
    AccountView, CheckOrganizationPermissionResponse, CreateOrganizationResponse,
    ListOrganizationMembersResponse, ListOrganizationsResponse,
};

pub(super) fn app_ready() {
    tracing::info!("tui_witness op=app kind=ready");
}

pub(super) fn sign_up_success(account: &AccountView, has_token: bool) {
    tracing::info!(
        "tui_witness op=sign_up kind=success account_id={} session_token_present={}",
        account.id,
        has_token
    );
}

pub(super) fn sign_in_success(account: &AccountView, has_token: bool) {
    tracing::info!(
        "tui_witness op=sign_in kind=success account_id={} session_token_present={}",
        account.id,
        has_token
    );
}

pub(super) fn accept_invitation_success(response: &AcceptInvitationCookieResponse) {
    tracing::info!(
        "tui_witness op=accept_invitation kind=success account_id={} joined_org={} session_token_present={}",
        response.account.id,
        response.joined_org,
        response.has_token
    );
}

pub(super) fn create_organization_success(response: &CreateOrganizationResponse) {
    let permissions = response
        .granted_permissions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let capabilities = response
        .capabilities
        .iter()
        .map(|capability| {
            format!(
                "{}:{}:{}",
                capability.permission, capability.key, capability.allowed
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let source_event_id = response
        .source_event
        .as_ref()
        .map_or_else(|| "<none>".to_owned(), |event| event.event_id.clone());
    let source_event_cursor = response
        .source_event
        .as_ref()
        .map_or_else(|| "<none>".to_owned(), |event| event.cursor.clone());
    tracing::info!(
        "tui_witness op=create_organization kind=success organization_id={} granted_permissions={} initial_project_count={} proof_behavior_id={} source_event={}.{} source_event_id={} source_event_cursor={} capabilities={}",
        response.organization.id,
        permissions,
        response.initial_project_count,
        response.proof_link.behavior_id,
        response.source_link.event_family,
        response.source_link.event_kind,
        source_event_id,
        source_event_cursor,
        capabilities
    );
}

pub(super) fn list_organizations_success(response: &ListOrganizationsResponse) {
    let next_cursor = response
        .next_cursor
        .map_or_else(|| "<none>".to_owned(), |cursor| cursor.to_string());
    let freshness_cursor = response
        .freshness
        .cursor
        .clone()
        .unwrap_or_else(|| "<none>".to_owned());
    let freshness_checkpoint = response
        .freshness
        .checkpoint
        .clone()
        .unwrap_or_else(|| "<none>".to_owned());
    tracing::info!(
        "tui_witness op=list_organizations kind=success count={} next_cursor={} freshness_projection={} freshness_generated_at={} freshness_cursor={} freshness_checkpoint={}",
        response.organizations.len(),
        next_cursor,
        response.freshness.projection,
        response.freshness.generated_at.to_rfc3339(),
        freshness_cursor,
        freshness_checkpoint
    );
    for org in &response.organizations {
        let capabilities = org
            .capabilities
            .iter()
            .map(|capability| {
                format!(
                    "{}:{}:{}",
                    capability.permission, capability.key, capability.allowed
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        tracing::info!(
            "tui_witness op=list_organizations kind=row organization_id={} name={:?} capabilities={}",
            org.id,
            org.name.as_str(),
            capabilities
        );
    }
}

pub(super) fn list_organization_members_success(response: &ListOrganizationMembersResponse) {
    let next_cursor = response
        .next_cursor
        .map_or_else(|| "<none>".to_owned(), |cursor| cursor.to_string());
    tracing::info!(
        "tui_witness op=list_organization_members kind=success count={} next_cursor={}",
        response.members.len(),
        next_cursor,
    );
    for member in &response.members {
        let permissions = member
            .granted_permissions
            .iter()
            .map(|g| format!("{}:{}", g.permission, g.grant_source))
            .collect::<Vec<_>>()
            .join(",");
        tracing::info!(
            "tui_witness op=list_organization_members kind=row account_id={} identifier={} permissions={}",
            member.account_id,
            member.identifier,
            permissions,
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
