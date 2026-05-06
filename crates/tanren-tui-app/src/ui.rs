//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_app_services::organization::CreateOrganizationOutput;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason,
    CreateOrganizationRequest, OrganizationAdminOperation, SignInRequest, SignInResponse,
    SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{
    Email, InvitationToken, OrgId, OrgPermission, OrganizationName, ValidationError,
};
use uuid::Uuid;

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
    vec![]
}

pub(crate) fn sign_up_outcome(response: &SignUpResponse) -> OutcomeView {
    OutcomeView {
        title: "Account created",
        lines: vec![
            format!("account_id: {}", response.account.id),
            format!("session token: {}", response.session.token.expose_secret()),
        ],
    }
}

pub(crate) fn sign_in_outcome(response: &SignInResponse) -> OutcomeView {
    OutcomeView {
        title: "Signed in",
        lines: vec![
            format!("account_id: {}", response.account.id),
            format!("session token: {}", response.session.token.expose_secret()),
        ],
    }
}

pub(crate) fn accept_invitation_outcome(response: &AcceptInvitationResponse) -> OutcomeView {
    OutcomeView {
        title: "Invitation accepted",
        lines: vec![
            format!("account_id: {}", response.account.id),
            format!("joined org: {}", response.joined_org),
            format!("session token: {}", response.session.token.expose_secret()),
        ],
    }
}

pub(crate) fn create_organization_outcome(output: &CreateOrganizationOutput) -> OutcomeView {
    let permission_labels: Vec<String> = output
        .granted_permissions
        .iter()
        .map(|p| format_permission(*p))
        .collect();
    OutcomeView {
        title: "Organization created",
        lines: vec![
            format!("organization_id: {}", output.organization.id),
            format!("name: {}", output.organization.display_name),
            format!("project_count: {}", output.project_count),
            format!("permissions: {}", permission_labels.join(", ")),
        ],
    }
}

fn format_permission(p: OrgPermission) -> String {
    match p {
        OrgPermission::InviteMembers => "invite_members".to_owned(),
        OrgPermission::ManageAccess => "manage_access".to_owned(),
        OrgPermission::Configure => "configure".to_owned(),
        OrgPermission::SetPolicy => "set_policy".to_owned(),
        OrgPermission::Delete => "delete".to_owned(),
        _ => "unknown".to_owned(),
    }
}

pub(crate) fn format_failure(reason: AccountFailureReason) -> String {
    format!("{}: {}", reason.code(), reason.summary())
}

pub(crate) fn render_error(err: AppServiceError) -> String {
    match err {
        AppServiceError::Account(reason) => format_failure(reason),
        AppServiceError::Organization(reason) => {
            format!("{}: {}", reason.code(), reason.summary())
        }
        AppServiceError::InvalidInput(message) => format!("validation_failed: {message}"),
        AppServiceError::Store(err) => format!("internal_error: {err}"),
        _ => "internal_error: unknown app-service failure".to_owned(),
    }
}

fn validation_message(err: &ValidationError) -> String {
    format!("validation_failed: {err}")
}

pub(crate) fn parse_sign_up(state: &FormState) -> Result<SignUpRequest, String> {
    let email = Email::parse(state.value(0)).map_err(|e| validation_message(&e))?;
    let password = SecretString::from(state.value(1).to_owned());
    let display_name = state.value(2).to_owned();
    Ok(SignUpRequest {
        email,
        password,
        display_name,
    })
}

pub(crate) fn parse_sign_in(state: &FormState) -> Result<SignInRequest, String> {
    let email = Email::parse(state.value(0)).map_err(|e| validation_message(&e))?;
    let password = SecretString::from(state.value(1).to_owned());
    Ok(SignInRequest { email, password })
}

pub(crate) fn parse_accept_invitation(
    state: &FormState,
) -> Result<AcceptInvitationRequest, String> {
    let invitation_token =
        InvitationToken::parse(state.value(0)).map_err(|e| validation_message(&e))?;
    let email = Email::parse(state.value(1)).map_err(|e| validation_message(&e))?;
    let password = SecretString::from(state.value(2).to_owned());
    let display_name = state.value(3).to_owned();
    Ok(AcceptInvitationRequest {
        invitation_token,
        email,
        password,
        display_name,
    })
}

pub(crate) fn parse_create_organization(
    state: &FormState,
) -> Result<CreateOrganizationRequest, String> {
    let name = OrganizationName::parse(state.value(0)).map_err(|e| validation_message(&e))?;
    Ok(CreateOrganizationRequest { name })
}

pub(crate) fn org_admin_probe_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Organization id (UUID)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Operation (invite_members, manage_access, configure, set_policy, delete)",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn parse_org_admin_probe(
    state: &FormState,
) -> Result<(OrgId, OrganizationAdminOperation), String> {
    let org_uuid =
        Uuid::parse_str(state.value(0)).map_err(|e| format!("validation_failed: {e}"))?;
    let org_id = OrgId::new(org_uuid);
    let operation = parse_tui_admin_operation(state.value(1))?;
    Ok((org_id, operation))
}

fn parse_tui_admin_operation(raw: &str) -> Result<OrganizationAdminOperation, String> {
    match raw.trim() {
        "invite_members" => Ok(OrganizationAdminOperation::InviteMembers),
        "manage_access" => Ok(OrganizationAdminOperation::ManageAccess),
        "configure" => Ok(OrganizationAdminOperation::Configure),
        "set_policy" => Ok(OrganizationAdminOperation::SetPolicy),
        "delete" => Ok(OrganizationAdminOperation::Delete),
        _ => Err(format!(
            "validation_failed: unknown operation '{raw}'; \
             expected invite_members, manage_access, configure, set_policy, or delete"
        )),
    }
}

pub(crate) fn operation_label(op: OrganizationAdminOperation) -> &'static str {
    match op {
        OrganizationAdminOperation::InviteMembers => "invite_members",
        OrganizationAdminOperation::ManageAccess => "manage_access",
        OrganizationAdminOperation::Configure => "configure",
        OrganizationAdminOperation::SetPolicy => "set_policy",
        OrganizationAdminOperation::Delete => "delete",
        _ => "unknown",
    }
}

pub(crate) fn list_organizations_outcome(orgs: &[(String, String, u64)]) -> OutcomeView {
    let mut lines = vec!["Available organizations:".to_owned()];
    if orgs.is_empty() {
        lines.push("(none)".to_owned());
    } else {
        for (id, name, count) in orgs {
            lines.push(format!("org_id={id} name={name} project_count={count}"));
        }
    }
    OutcomeView {
        title: "Organizations",
        lines,
    }
}

pub(crate) fn org_admin_probe_outcome(
    org_id: OrgId,
    operation: OrganizationAdminOperation,
) -> OutcomeView {
    OutcomeView {
        title: "Authorized",
        lines: vec![
            format!("org_id: {org_id}"),
            format!("operation: {}", operation_label(operation)),
            "authorized: true".to_owned(),
        ],
    }
}
