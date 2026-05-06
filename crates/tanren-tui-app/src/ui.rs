//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason,
    JoinOrganizationResponse, MembershipDepartureResponse, SignInRequest, SignInResponse,
    SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken, OrgId, ValidationError};
use uuid::Uuid;

use crate::{DepartureKind, FormField, FormState, OutcomeView};

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

pub(crate) fn format_failure(reason: AccountFailureReason) -> String {
    format!("{}: {}", reason.code(), reason.summary())
}

pub(crate) fn render_error(err: AppServiceError) -> String {
    match err {
        AppServiceError::Account(reason) => format_failure(reason),
        AppServiceError::InvalidInput(message) => format!("validation_failed: {message}"),
        AppServiceError::Store(err) => format!("internal_error: {err}"),
        _ => "internal_error: unknown app-service failure".to_owned(),
    }
}

fn validation_message(err: &ValidationError) -> String {
    format!("validation_failed: {err}")
}

pub(crate) fn unauthenticated_error() -> String {
    format_failure(AccountFailureReason::Unauthenticated)
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

pub(crate) fn join_organization_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Invitation token",
        secret: false,
        value: String::new(),
    }]
}

pub(crate) fn join_organization_outcome(response: &JoinOrganizationResponse) -> OutcomeView {
    OutcomeView {
        title: "Joined organization",
        lines: vec![
            format!("joined org: {}", response.joined_org),
            format!("permissions: {}", response.membership_permissions),
            "project access: (none)".to_owned(),
        ],
    }
}

pub(crate) fn parse_join_invitation(state: &FormState) -> Result<InvitationToken, String> {
    InvitationToken::parse(state.value(0)).map_err(|e| validation_message(&e))
}

pub(crate) fn leave_organization_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Organization ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Acknowledge in-flight work (yes/no)",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn remove_member_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Organization ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Target account ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Acknowledge in-flight work (yes/no)",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn parse_org_id(raw: &str) -> Result<OrgId, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("validation_failed: organization ID is required".to_owned());
    }
    Uuid::parse_str(trimmed)
        .map(OrgId::new)
        .map_err(|e| format!("validation_failed: invalid organization ID: {e}"))
}

pub(crate) fn parse_account_id(raw: &str) -> Result<AccountId, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("validation_failed: account ID is required".to_owned());
    }
    Uuid::parse_str(trimmed)
        .map(AccountId::new)
        .map_err(|e| format!("validation_failed: invalid account ID: {e}"))
}

pub(crate) fn parse_acknowledge(raw: &str) -> Result<bool, String> {
    let trimmed = raw.trim().to_lowercase();
    match trimmed.as_str() {
        "yes" | "y" | "true" | "1" => Ok(true),
        "no" | "n" | "false" | "0" | "" => Ok(false),
        _ => Err("validation_failed: acknowledge must be yes or no".to_owned()),
    }
}

pub(crate) fn parse_departure_leave(state: &FormState) -> Result<(OrgId, bool), String> {
    let org_id = parse_org_id(state.value(0))?;
    let ack = parse_acknowledge(state.value(1))?;
    Ok((org_id, ack))
}

pub(crate) fn parse_departure_remove(
    state: &FormState,
) -> Result<(OrgId, AccountId, bool), String> {
    let org_id = parse_org_id(state.value(0))?;
    let target = parse_account_id(state.value(1))?;
    let ack = parse_acknowledge(state.value(2))?;
    Ok((org_id, target, ack))
}

pub(crate) fn departure_outcome(
    response: &MembershipDepartureResponse,
    dk: DepartureKind,
) -> OutcomeView {
    let title = match dk {
        DepartureKind::Remove => "Member removal",
        DepartureKind::Leave => "Leave organization",
    };
    let mut lines = Vec::new();
    if response.completed {
        if let Some(org) = response.departed_org {
            lines.push(format!("departed org: {org}"));
        }
        lines.push("status: completed".to_owned());
    } else {
        lines.push("status: preview — in-flight work found".to_owned());
        let count = response.in_flight_work.len();
        lines.push(format!("in-flight items: {count}"));
        lines.push("Submit again with acknowledgement to complete.".to_owned());
    }
    if !response.selectable_organizations.is_empty() {
        lines.push("remaining organizations:".to_owned());
        for org in &response.selectable_organizations {
            lines.push(format!("  - {} ({})", org.org_id, org.permissions));
        }
    }
    OutcomeView { title, lines }
}
