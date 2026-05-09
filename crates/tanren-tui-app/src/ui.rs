//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, MyPermissionEntry,
    MyPermissionsResponse, SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{
    Email, InvitationToken, PermissionGrantSource, PolicyConstraintSource, ValidationError,
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

pub(crate) fn my_permissions_outcome(response: &MyPermissionsResponse) -> OutcomeView {
    let mut lines = Vec::new();
    if response.organizations.is_empty() && response.projects.is_empty() {
        lines.push("permissions=none".to_owned());
    } else {
        lines.push("organizations".to_owned());
        for organization in &response.organizations {
            lines.push(format!("  org_id={}", organization.org_id));
            append_permission_lines(&mut lines, &organization.permissions);
        }
        lines.push("projects".to_owned());
        for project in &response.projects {
            lines.push(format!("  project_id={}", project.project_id));
            append_permission_lines(&mut lines, &project.permissions);
        }
    }
    OutcomeView {
        title: "My permissions",
        lines,
    }
}

fn append_permission_lines(lines: &mut Vec<String>, permissions: &[MyPermissionEntry]) {
    for permission in permissions {
        let (constraint_reason, constraint_source) =
            permission.policy_constraint.as_ref().map_or_else(
                || ("none".to_owned(), "none".to_owned()),
                |constraint| {
                    (
                        constraint.reason.to_string(),
                        format_constraint_source(constraint.source),
                    )
                },
            );
        lines.push(format!(
            "    permission={} state={:?} source={} constraint_reason={} constraint_source={}",
            permission.permission,
            permission.effective_state,
            format_grant_source(&permission.grant_source),
            constraint_reason,
            constraint_source,
        ));
    }
}

fn format_grant_source(source: &PermissionGrantSource) -> String {
    match source {
        PermissionGrantSource::Direct => "direct".to_owned(),
        PermissionGrantSource::RoleTemplate { role_template } => {
            format!("role_template:{role_template}")
        }
    }
}

fn format_constraint_source(source: PolicyConstraintSource) -> String {
    match source {
        PolicyConstraintSource::OrganizationPolicy => "organization_policy".to_owned(),
        PolicyConstraintSource::ProjectPolicy => "project_policy".to_owned(),
    }
}

pub(crate) fn format_failure(reason: AccountFailureReason) -> String {
    format!("{}: {}", reason.code(), reason.summary())
}

pub(crate) fn render_error(err: AppServiceError) -> String {
    match err {
        AppServiceError::Account(reason) => format_failure(reason),
        AppServiceError::Permissions(reason) => {
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
