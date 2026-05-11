//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use crate::app::api::{
    AcceptInvitationInput, CheckOrganizationPermissionInput, CreateOrganizationInput, SignInInput,
    SignUpInput,
};
use secrecy::SecretString;
use tanren_contract::{
    AccountFailureReason, AccountView, CheckOrganizationPermissionResponse,
    CreateOrganizationResponse, ListOrganizationsResponse,
};

use crate::{FormField, FormState, OutcomeView};

pub(crate) fn sign_up_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Email",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Password",
            secret: true,
            value: String::new(),
        },
        FormField {
            label: "Display name",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn sign_in_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Email",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Password",
            secret: true,
            value: String::new(),
        },
    ]
}

pub(crate) fn accept_invitation_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Invitation token",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Email",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Password",
            secret: true,
            value: String::new(),
        },
        FormField {
            label: "Display name",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn create_organization_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Organization name",
        secret: false,
        value: String::new(),
    }]
}

pub(crate) fn list_organizations_fields() -> Vec<FormField> {
    Vec::new()
}

pub(crate) fn check_organization_permission_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Organization ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Permission",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn sign_up_outcome(account: &AccountView, has_token: bool) -> OutcomeView {
    OutcomeView {
        title: "Account created",
        lines: vec![
            format!("account_id: {}", account.id),
            format!("session established: {has_token}"),
        ],
    }
}

pub(crate) fn sign_in_outcome(account: &AccountView, has_token: bool) -> OutcomeView {
    OutcomeView {
        title: "Signed in",
        lines: vec![
            format!("account_id: {}", account.id),
            format!("session established: {has_token}"),
        ],
    }
}

pub(crate) fn accept_invitation_outcome(
    account: &AccountView,
    joined_org: &str,
    has_token: bool,
) -> OutcomeView {
    OutcomeView {
        title: "Invitation accepted",
        lines: vec![
            format!("account_id: {}", account.id),
            format!("joined org: {joined_org}"),
            format!("session established: {has_token}"),
        ],
    }
}

pub(crate) fn create_organization_outcome(response: &CreateOrganizationResponse) -> OutcomeView {
    let granted = response
        .granted_permissions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
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
        .join(", ");
    let source_event_id = response
        .source_event
        .as_ref()
        .map_or_else(|| "<none>".to_owned(), |event| event.event_id.clone());
    let source_event_cursor = response
        .source_event
        .as_ref()
        .map_or_else(|| "<none>".to_owned(), |event| event.cursor.clone());
    OutcomeView {
        title: "Organization created",
        lines: vec![
            format!("organization_id: {}", response.organization.id),
            format!("name: {}", response.organization.name),
            format!("granted_permissions: {granted}"),
            format!("capabilities: {capabilities}"),
            format!("initial_project_count: {}", response.initial_project_count),
            format!(
                "project_total_count: {}",
                response.project_summary.total_count
            ),
            format!("proof_behavior_id: {}", response.proof_link.behavior_id),
            format!(
                "source_event: {}.{}",
                response.source_link.event_family, response.source_link.event_kind
            ),
            format!("source_event_id: {source_event_id}"),
            format!("source_event_cursor: {source_event_cursor}"),
        ],
    }
}

pub(crate) fn list_organizations_outcome(response: &ListOrganizationsResponse) -> OutcomeView {
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
    let mut lines = vec![
        format!("count: {}", response.organizations.len()),
        format!("next_cursor: {next_cursor}"),
        format!("freshness_projection: {}", response.freshness.projection),
        format!(
            "freshness_generated_at: {}",
            response.freshness.generated_at.to_rfc3339()
        ),
        format!("freshness_cursor: {freshness_cursor}"),
        format!("freshness_checkpoint: {freshness_checkpoint}"),
    ];
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
            .join(", ");
        lines.push(format!(
            "organization_id: {} name: {} capabilities: {capabilities}",
            org.id, org.name
        ));
    }
    OutcomeView {
        title: "Organizations",
        lines,
    }
}

pub(crate) fn check_organization_permission_outcome(
    response: &CheckOrganizationPermissionResponse,
) -> OutcomeView {
    OutcomeView {
        title: "Permission granted",
        lines: vec![
            format!("account_id: {}", response.account_id),
            format!("org_id: {}", response.org_id),
            format!("permission: {}", response.permission),
        ],
    }
}

pub(crate) fn format_failure(reason: AccountFailureReason) -> String {
    format!("{}: {}", reason.code(), reason.summary())
}

pub(crate) fn auth_required_message() -> String {
    format_failure(AccountFailureReason::AuthRequired)
}

pub(crate) fn permission_denied_message() -> String {
    format_failure(AccountFailureReason::PermissionDenied)
}

pub(crate) fn parse_sign_up(state: &FormState) -> Result<SignUpInput, String> {
    let email = required_field("email", state.value(0))?;
    let password = required_field("password", state.value(1))?;
    let display_name = required_field("display_name", state.value(2))?;
    Ok(SignUpInput {
        email,
        password: SecretString::from(password),
        display_name,
    })
}

pub(crate) fn parse_sign_in(state: &FormState) -> Result<SignInInput, String> {
    let email = required_field("email", state.value(0))?;
    let password = required_field("password", state.value(1))?;
    Ok(SignInInput {
        email,
        password: SecretString::from(password),
    })
}

pub(crate) fn parse_accept_invitation(state: &FormState) -> Result<AcceptInvitationInput, String> {
    let invitation_token = required_field("invitation_token", state.value(0))?;
    let email = required_field("email", state.value(1))?;
    let password = required_field("password", state.value(2))?;
    let display_name = required_field("display_name", state.value(3))?;
    Ok(AcceptInvitationInput {
        invitation_token,
        email,
        password: SecretString::from(password),
        display_name,
    })
}

pub(crate) fn parse_create_organization(
    state: &FormState,
) -> Result<CreateOrganizationInput, String> {
    let name = required_field("organization_name", state.value(0))?;
    Ok(CreateOrganizationInput { name })
}

pub(crate) fn parse_check_organization_permission(
    state: &FormState,
) -> Result<CheckOrganizationPermissionInput, String> {
    let org_id = required_field("org_id", state.value(0))?;
    let permission = required_field("permission", state.value(1))?;
    Ok(CheckOrganizationPermissionInput { org_id, permission })
}

fn required_field(label: &str, raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("validation_failed: {label} must not be empty"));
    }
    Ok(trimmed.to_owned())
}
