//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use std::str::FromStr;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason,
    CheckOrganizationPermissionRequest, CheckOrganizationPermissionResponse,
    CreateOrganizationRequest, CreateOrganizationResponse, ListOrganizationsRequest,
    ListOrganizationsResponse, SessionView, SignInRequest, SignInResponse, SignUpRequest,
    SignUpResponse,
};
use tanren_identity_policy::{
    Email, InvitationToken, OrgId, OrganizationName, OrganizationPermission, ValidationError,
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

pub(crate) fn create_organization_outcome(response: &CreateOrganizationResponse) -> OutcomeView {
    let granted = response
        .granted_permissions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    OutcomeView {
        title: "Organization created",
        lines: vec![
            format!("organization_id: {}", response.organization.id),
            format!("name: {}", response.organization.name),
            format!("granted_permissions: {granted}"),
        ],
    }
}

pub(crate) fn list_organizations_outcome(response: &ListOrganizationsResponse) -> OutcomeView {
    let mut lines = vec![format!("count: {}", response.organizations.len())];
    for org in &response.organizations {
        lines.push(format!("organization_id: {} name: {}", org.id, org.name));
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

pub(crate) fn render_error(err: AppServiceError) -> String {
    match err {
        AppServiceError::Account(reason) => format_failure(reason),
        AppServiceError::InvalidInput(message) => {
            if message == "idempotency_conflict" {
                "idempotency_conflict: The supplied idempotency key conflicts with a prior request."
                    .to_owned()
            } else {
                format!("validation_failed: {message}")
            }
        }
        AppServiceError::Store(err) => format!("internal_error: {err}"),
        _ => "internal_error: unknown app-service failure".to_owned(),
    }
}

pub(crate) fn auth_required_message() -> String {
    format_failure(AccountFailureReason::AuthRequired)
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
    session: &SessionView,
) -> Result<CreateOrganizationRequest, String> {
    let name = OrganizationName::parse(state.value(0)).map_err(|e| validation_message(&e))?;
    Ok(CreateOrganizationRequest {
        session_token: session.token.clone(),
        account_id: session.account_id,
        name,
        idempotency_key: None,
    })
}

pub(crate) fn parse_list_organizations(session: &SessionView) -> ListOrganizationsRequest {
    ListOrganizationsRequest {
        session_token: session.token.clone(),
        account_id: session.account_id,
    }
}

pub(crate) fn parse_check_organization_permission(
    state: &FormState,
    session: &SessionView,
) -> Result<CheckOrganizationPermissionRequest, String> {
    let org_id = parse_org_id(state.value(0))?;
    let permission = parse_permission(state.value(1))?;
    Ok(CheckOrganizationPermissionRequest {
        session_token: session.token.clone(),
        account_id: session.account_id,
        org_id,
        permission,
    })
}

fn parse_org_id(raw: &str) -> Result<OrgId, String> {
    let uuid = Uuid::parse_str(raw).map_err(|e| format!("validation_failed: {e}"))?;
    Ok(OrgId::from(uuid))
}

fn parse_permission(raw: &str) -> Result<OrganizationPermission, String> {
    OrganizationPermission::from_str(raw.trim()).map_err(|_| {
        let expected = OrganizationPermission::ALL
            .into_iter()
            .map(OrganizationPermission::as_str)
            .collect::<Vec<_>>()
            .join("|");
        format!("validation_failed: permission must be {expected}")
    })
}
