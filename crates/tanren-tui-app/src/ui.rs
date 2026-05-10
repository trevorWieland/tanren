//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AcceptInvitationResponseBearer,
    AccountFailureReason, ActiveProjectView, ConnectProjectRepositoryRequest,
    ConnectProjectRepositoryResponse, CreateProjectRequest, CreateProjectResponse,
    ListVisibleProjectsRequest, ProjectCollectionView, ProjectFailureReason, ProjectPageRequest,
    SignInRequest, SignInResponse, SignInResponseBearer, SignUpRequest, SignUpResponse,
    SignUpResponseBearer,
};
use tanren_identity_policy::{
    AccountId, DesignatedHost, Email, InvitationToken, RepositoryRef, SessionToken, ValidationError,
};

#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedProjectRequest<T> {
    pub(crate) session_token: Option<SessionToken>,
    pub(crate) request: T,
}

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
            label: "Session token (blank=reuse)",
            secret: true,
            value: String::new(),
        },
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
            label: "Session token (blank=reuse)",
            secret: true,
            value: String::new(),
        },
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
    vec![
        FormField {
            label: "Session token (blank=reuse)",
            secret: true,
            value: String::new(),
        },
        FormField {
            label: "Owning account id",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn active_project_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Session token (blank=reuse)",
            secret: true,
            value: String::new(),
        },
        FormField {
            label: "Owning account id",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn sign_up_outcome(response: &SignUpResponse) -> OutcomeView {
    let bearer = SignUpResponseBearer::from_sign_up_response(response);
    OutcomeView {
        title: "Account created",
        lines: vec![
            format!("account_id: {}", bearer.account.id),
            format!("session token: {}", bearer.session.token.expose_secret()),
        ],
    }
}

pub(crate) fn sign_in_outcome(response: &SignInResponse) -> OutcomeView {
    let bearer = SignInResponseBearer::from_sign_in_response(response);
    OutcomeView {
        title: "Signed in",
        lines: vec![
            format!("account_id: {}", bearer.account.id),
            format!("session token: {}", bearer.session.token.expose_secret()),
        ],
    }
}

pub(crate) fn accept_invitation_outcome(response: &AcceptInvitationResponse) -> OutcomeView {
    let bearer = AcceptInvitationResponseBearer::from_accept_invitation_response(response);
    OutcomeView {
        title: "Invitation accepted",
        lines: vec![
            format!("account_id: {}", bearer.account.id),
            format!("joined org: {}", bearer.joined_org),
            format!("session token: {}", bearer.session.token.expose_secret()),
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

pub(crate) fn render_project_error(err: AppServiceError) -> String {
    match err {
        AppServiceError::Project(reason) => format_project_failure(reason),
        AppServiceError::InvalidInput(message) => format!("validation_failed: {message}"),
        AppServiceError::Store(err) => {
            tracing::error!(error = ?err, "tui project request failed due to store error");
            "internal_error: Tanren encountered an internal project error.".to_owned()
        }
        other => {
            tracing::error!(error = ?other, "tui project request failed unexpectedly");
            "internal_error: Tanren encountered an internal project error.".to_owned()
        }
    }
}

fn format_project_failure(reason: ProjectFailureReason) -> String {
    match reason {
        ProjectFailureReason::AuthRequired
        | ProjectFailureReason::DuplicateRepository
        | ProjectFailureReason::NoAccess
        | ProjectFailureReason::ValidationFailed
        | ProjectFailureReason::ProviderUnavailable
        | ProjectFailureReason::ProviderFailure => {
            format!("{}: {}", reason.code(), reason.summary())
        }
        _ => format!("{}: {}", reason.code(), reason.summary()),
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
) -> Result<AuthenticatedProjectRequest<ConnectProjectRepositoryRequest>, String> {
    let session_token = parse_optional_session_token(state.value(0))?;
    let owning_account_id = parse_account_id(state.value(1))?;
    let repository = RepositoryRef::parse(state.value(2)).map_err(|e| validation_message(&e))?;
    let select_as_active = parse_select_as_active(state.value(3))?;
    Ok(AuthenticatedProjectRequest {
        session_token,
        request: ConnectProjectRepositoryRequest {
            owning_account_id,
            repository,
            select_as_active,
        },
    })
}

pub(crate) fn parse_create_project(
    state: &FormState,
) -> Result<AuthenticatedProjectRequest<CreateProjectRequest>, String> {
    let session_token = parse_optional_session_token(state.value(0))?;
    let owning_account_id = parse_account_id(state.value(1))?;
    let repository = RepositoryRef::parse(state.value(2)).map_err(|e| validation_message(&e))?;
    let designated_host =
        DesignatedHost::parse(state.value(3)).map_err(|e| validation_message(&e))?;
    let select_as_active = parse_select_as_active(state.value(4))?;
    Ok(AuthenticatedProjectRequest {
        session_token,
        request: CreateProjectRequest {
            owning_account_id,
            repository,
            designated_host,
            select_as_active,
        },
    })
}

pub(crate) fn parse_list_projects(
    state: &FormState,
) -> Result<AuthenticatedProjectRequest<ListVisibleProjectsRequest>, String> {
    let session_token = parse_optional_session_token(state.value(0))?;
    let owning_account_id = parse_account_id(state.value(1))?;
    Ok(AuthenticatedProjectRequest {
        session_token,
        request: ListVisibleProjectsRequest {
            owning_account_id,
            page: ProjectPageRequest::default(),
        },
    })
}

pub(crate) fn parse_active_project(
    state: &FormState,
) -> Result<AuthenticatedProjectRequest<tanren_contract::ActiveProjectRequest>, String> {
    let session_token = parse_optional_session_token(state.value(0))?;
    let owning_account_id = parse_account_id(state.value(1))?;
    Ok(AuthenticatedProjectRequest {
        session_token,
        request: tanren_contract::ActiveProjectRequest { owning_account_id },
    })
}

fn parse_account_id(raw: &str) -> Result<AccountId, String> {
    AccountId::parse(raw.trim()).map_err(|err| format!("validation_failed: {err}"))
}

fn parse_optional_session_token(raw: &str) -> Result<Option<SessionToken>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    SessionToken::parse(trimmed)
        .map(Some)
        .map_err(|err| format!("validation_failed: {err}"))
}

fn parse_select_as_active(raw: &str) -> Result<bool, String> {
    raw.trim()
        .parse::<bool>()
        .map_err(|_| "validation_failed: select_as_active must be true or false".to_owned())
}
