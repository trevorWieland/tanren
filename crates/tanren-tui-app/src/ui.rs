//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, ActiveProjectView,
    ConnectProjectRepositoryRequest, ConnectProjectRepositoryResponse, CreateProjectRequest,
    CreateProjectResponse, ListVisibleProjectsRequest, ProjectCollectionView, ProjectPageRequest,
    SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{
    AccountId, DesignatedHost, Email, InvitationToken, RepositoryRef, ValidationError,
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

pub(crate) fn connect_repository_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Owning account id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Repository (owner/name)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Select as active (true/false)",
            secret: false,
            value: "true".to_owned(),
        },
    ]
}

pub(crate) fn create_project_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Owning account id",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Repository (owner/name)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Designated host",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Select as active (true/false)",
            secret: false,
            value: "true".to_owned(),
        },
    ]
}

pub(crate) fn list_projects_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Owning account id",
        secret: false,
        value: String::new(),
    }]
}

pub(crate) fn active_project_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Owning account id",
        secret: false,
        value: String::new(),
    }]
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

pub(crate) fn connect_repository_outcome(
    response: &ConnectProjectRepositoryResponse,
) -> OutcomeView {
    let project = &response.project;
    OutcomeView {
        title: "Repository connected",
        lines: vec![
            format!("project_id: {}", project.id),
            format!("repository: {}", project.repository.repository),
            format!("active: {}", project.selection.is_active),
            format!(
                "counts: specs={} milestones={} initiatives={}",
                project.counts.specs, project.counts.milestones, project.counts.initiatives
            ),
        ],
    }
}

pub(crate) fn create_project_outcome(response: &CreateProjectResponse) -> OutcomeView {
    let project = &response.project;
    OutcomeView {
        title: "Project created",
        lines: vec![
            format!("project_id: {}", project.id),
            format!("repository: {}", project.repository.repository),
            format!("active: {}", project.selection.is_active),
            format!(
                "counts: specs={} milestones={} initiatives={}",
                project.counts.specs, project.counts.milestones, project.counts.initiatives
            ),
        ],
    }
}

pub(crate) fn list_projects_outcome(response: &ProjectCollectionView) -> OutcomeView {
    let mut lines = vec![format!(
        "owning_account_id: {} (projects={})",
        response.owning_account_id,
        response.projects.len()
    )];
    if response.projects.is_empty() {
        lines.push("projects: none".to_owned());
    } else {
        for project in &response.projects {
            lines.push(format!(
                "{} repo={} active={} specs={} milestones={} initiatives={}",
                project.id,
                project.repository.repository,
                project.selection.is_active,
                project.counts.specs,
                project.counts.milestones,
                project.counts.initiatives
            ));
        }
    }
    OutcomeView {
        title: "Visible projects",
        lines,
    }
}

pub(crate) fn active_project_outcome(response: &ActiveProjectView) -> OutcomeView {
    let mut lines = vec![format!("owning_account_id: {}", response.owning_account_id)];
    match response.active_project.as_ref() {
        Some(project) => {
            lines.push(format!("project_id: {}", project.id));
            lines.push(format!("repository: {}", project.repository.repository));
            lines.push(format!("active: {}", project.selection.is_active));
            lines.push(format!(
                "counts: specs={} milestones={} initiatives={}",
                project.counts.specs, project.counts.milestones, project.counts.initiatives
            ));
        }
        None => lines.push("active_project: none".to_owned()),
    }
    OutcomeView {
        title: "Active project",
        lines,
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
    // The user supplies the email directly; the previous implementation
    // synthesised it from the invitation token, which broke any token
    // containing `@` (the resulting "<token>@invitation.tanren" had two
    // `@` characters and Email::parse rejected it before the request
    // ever reached `accept_invitation`). Codex P2 review on PR #133.
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

pub(crate) fn parse_connect_repository(
    state: &FormState,
) -> Result<ConnectProjectRepositoryRequest, String> {
    let owning_account_id = parse_account_id(state.value(0))?;
    let repository = RepositoryRef::parse(state.value(1)).map_err(|e| validation_message(&e))?;
    let select_as_active = parse_select_as_active(state.value(2))?;
    Ok(ConnectProjectRepositoryRequest {
        owning_account_id,
        repository,
        select_as_active,
    })
}

pub(crate) fn parse_create_project(state: &FormState) -> Result<CreateProjectRequest, String> {
    let owning_account_id = parse_account_id(state.value(0))?;
    let repository = RepositoryRef::parse(state.value(1)).map_err(|e| validation_message(&e))?;
    let designated_host =
        DesignatedHost::parse(state.value(2)).map_err(|e| validation_message(&e))?;
    let select_as_active = parse_select_as_active(state.value(3))?;
    Ok(CreateProjectRequest {
        owning_account_id,
        repository,
        designated_host,
        select_as_active,
    })
}

pub(crate) fn parse_list_projects(state: &FormState) -> Result<ListVisibleProjectsRequest, String> {
    let owning_account_id = parse_account_id(state.value(0))?;
    Ok(ListVisibleProjectsRequest {
        owning_account_id,
        page: ProjectPageRequest::default(),
    })
}

pub(crate) fn parse_active_project(
    state: &FormState,
) -> Result<tanren_contract::ActiveProjectRequest, String> {
    let owning_account_id = parse_account_id(state.value(0))?;
    Ok(tanren_contract::ActiveProjectRequest { owning_account_id })
}

fn parse_account_id(raw: &str) -> Result<AccountId, String> {
    let parsed = Uuid::parse_str(raw.trim())
        .map_err(|err| format!("validation_failed: invalid account id: {err}"))?;
    Ok(AccountId::new(parsed))
}

fn parse_select_as_active(raw: &str) -> Result<bool, String> {
    raw.trim()
        .parse::<bool>()
        .map_err(|_| "validation_failed: select_as_active must be true or false".to_owned())
}
