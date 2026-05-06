//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, ActiveProjectView,
    ConnectProjectRequest, CreateProjectRequest, ProjectFailureReason, ProjectView, SignInRequest,
    SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken, ValidationError};
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

pub(crate) fn connect_project_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Account ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Project name",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Repository URL",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn create_project_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Account ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Project name",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Provider host",
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

pub(crate) fn connect_project_outcome(project: &ProjectView) -> OutcomeView {
    OutcomeView {
        title: "Project connected",
        lines: vec![
            format!("project_id: {}", project.id),
            format!("repository_id: {}", project.repository.id),
            format!("repository_url: {}", project.repository.url),
            "active: true".to_owned(),
            format!(
                "specs: {}  milestones: {}  initiatives: {}",
                project.content_counts.specs,
                project.content_counts.milestones,
                project.content_counts.initiatives,
            ),
        ],
    }
}

pub(crate) fn create_project_outcome(project: &ProjectView) -> OutcomeView {
    OutcomeView {
        title: "Project created",
        lines: vec![
            format!("project_id: {}", project.id),
            format!("repository_id: {}", project.repository.id),
            format!("repository_url: {}", project.repository.url),
            "active: true".to_owned(),
            format!(
                "specs: {}  milestones: {}  initiatives: {}",
                project.content_counts.specs,
                project.content_counts.milestones,
                project.content_counts.initiatives,
            ),
        ],
    }
}

pub(crate) fn active_project_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Account ID",
        secret: false,
        value: String::new(),
    }]
}

pub(crate) fn active_project_outcome(view: &ActiveProjectView) -> OutcomeView {
    let p = &view.project;
    OutcomeView {
        title: "Active project",
        lines: vec![
            format!("project_id: {}", p.id),
            format!("repository_id: {}", p.repository.id),
            format!("repository_url: {}", p.repository.url),
            format!("activated_at: {}", view.activated_at),
            "active: true".to_owned(),
            format!(
                "specs: {}  milestones: {}  initiatives: {}",
                p.content_counts.specs, p.content_counts.milestones, p.content_counts.initiatives,
            ),
        ],
    }
}

pub(crate) fn active_project_none_outcome() -> OutcomeView {
    OutcomeView {
        title: "Active project",
        lines: vec!["active: none".to_owned()],
    }
}

pub(crate) fn parse_active_project(state: &FormState) -> Result<AccountId, String> {
    parse_account_id(state, 0)
}

pub(crate) fn format_account_failure(reason: AccountFailureReason) -> String {
    format!("{}: {}", reason.code(), reason.summary())
}

pub(crate) fn format_project_failure(reason: ProjectFailureReason) -> String {
    format!("{}: {}", reason.code(), reason.summary())
}

pub(crate) fn render_error(err: AppServiceError) -> String {
    match err {
        AppServiceError::Account(reason) => format_account_failure(reason),
        AppServiceError::Project(reason) => format_project_failure(reason),
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

pub(crate) fn parse_account_id(state: &FormState, idx: usize) -> Result<AccountId, String> {
    let raw = state.value(idx);
    let uuid = Uuid::parse_str(raw).map_err(|_| format!("invalid account ID: {raw}"))?;
    Ok(AccountId::new(uuid))
}

pub(crate) fn parse_connect_project(
    state: &FormState,
) -> Result<(AccountId, ConnectProjectRequest), String> {
    let account_id = parse_account_id(state, 0)?;
    let name = state.value(1).to_owned();
    let repository_url = state.value(2).to_owned();
    Ok((
        account_id,
        ConnectProjectRequest {
            name,
            repository_url,
            org: None,
        },
    ))
}

pub(crate) fn parse_create_project(
    state: &FormState,
) -> Result<(AccountId, CreateProjectRequest), String> {
    let account_id = parse_account_id(state, 0)?;
    let name = state.value(1).to_owned();
    let provider_host = state.value(2).to_owned();
    Ok((
        account_id,
        CreateProjectRequest {
            name,
            provider_host,
            org: None,
        },
    ))
}
