//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason,
    ListOrganizationProjectsResponse, OrganizationSwitcher, SignInRequest, SignInResponse,
    SignUpRequest, SignUpResponse, SwitchActiveOrganizationRequest,
    SwitchActiveOrganizationResponse,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken, OrgId, ValidationError};

use crate::{FormAction, FormField, FormState, OutcomeView};

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

pub(crate) fn list_orgs_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Account ID",
        secret: false,
        value: String::new(),
    }]
}

pub(crate) fn switch_org_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Account ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Organization ID",
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

pub(crate) fn list_orgs_outcome(response: &OrganizationSwitcher) -> OutcomeView {
    let mut lines = vec![format!(
        "active_org: {}",
        format_opt_id(response.active_org.as_ref())
    )];
    if response.memberships.is_empty() {
        lines.push("No organizations (personal account).".to_owned());
    } else {
        for m in &response.memberships {
            let marker = match &response.active_org {
                Some(active) if *active == m.org_id => " (active)",
                _ => "",
            };
            lines.push(format!("  {} — {}{}", m.org_id, m.org_name, marker));
        }
    }
    OutcomeView {
        title: "Organizations",
        lines,
    }
}

pub(crate) fn switch_org_outcome(response: &SwitchActiveOrganizationResponse) -> OutcomeView {
    OutcomeView {
        title: "Active org switched",
        lines: vec![format!(
            "account_id: {}  active_org: {}",
            response.account.id,
            format_opt_id(response.account.org.as_ref()),
        )],
    }
}

pub(crate) fn list_org_projects_outcome(
    response: &ListOrganizationProjectsResponse,
) -> OutcomeView {
    let mut lines = Vec::new();
    if response.projects.is_empty() {
        lines.push("No projects in active organization.".to_owned());
    } else {
        for p in &response.projects {
            lines.push(format!("  {} — {} (org: {})", p.id, p.name, p.org));
        }
    }
    OutcomeView {
        title: "Org projects",
        lines,
    }
}

fn format_opt_id(id: Option<&OrgId>) -> String {
    match id {
        Some(org_id) => org_id.to_string(),
        None => "none".to_owned(),
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

pub(crate) fn parse_account_id(state: &FormState) -> Result<AccountId, String> {
    let uuid = state
        .value(0)
        .parse::<uuid::Uuid>()
        .map_err(|e| format!("validation_failed: {e}"))?;
    Ok(AccountId::new(uuid))
}

pub(crate) fn parse_account_and_org_id(
    state: &FormState,
) -> Result<(AccountId, SwitchActiveOrganizationRequest), String> {
    let account_uuid = state
        .value(0)
        .parse::<uuid::Uuid>()
        .map_err(|e| format!("validation_failed: {e}"))?;
    let org_uuid = state
        .value(1)
        .parse::<uuid::Uuid>()
        .map_err(|e| format!("validation_failed: {e}"))?;
    Ok((
        AccountId::new(account_uuid),
        SwitchActiveOrganizationRequest {
            org_id: OrgId::new(org_uuid),
        },
    ))
}

pub(crate) fn handle_form_key(state: &mut FormState, key: KeyEvent) -> Option<FormAction> {
    match key.code {
        KeyCode::Esc => Some(FormAction::Cancel),
        KeyCode::Enter => Some(FormAction::Submit),
        KeyCode::Tab | KeyCode::Down => {
            state.cycle_focus(true);
            None
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.cycle_focus(false);
            None
        }
        KeyCode::Backspace => {
            state.pop_char();
            None
        }
        KeyCode::Char(c) => {
            state.push_char(c);
            None
        }
        _ => None,
    }
}

pub(crate) fn is_press(key: &KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}
