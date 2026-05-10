//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use std::str::FromStr;
use tanren_app_services::AppServiceError;
use tanren_app_services::deployment_posture::SetDeploymentPostureError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, DeploymentPosture,
    DeploymentPostureCapability, DeploymentPostureUnavailableCapability,
    SetDeploymentPostureRequest, SetDeploymentPostureResponse, SignInRequest, SignInResponse,
    SignUpRequest, SignUpResponse, SupportedDeploymentPosture,
};
use tanren_identity_policy::{AccountId, Email, InvitationToken, ValidationError};

use crate::{FormField, FormState, OutcomeView};

pub(crate) fn sign_up_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Email",
            secret: false,
            read_only: false,
            value: String::new(),
        },
        FormField {
            label: "Password",
            secret: true,
            read_only: false,
            value: String::new(),
        },
        FormField {
            label: "Display name",
            secret: false,
            read_only: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn sign_in_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Email",
            secret: false,
            read_only: false,
            value: String::new(),
        },
        FormField {
            label: "Password",
            secret: true,
            read_only: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn accept_invitation_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Invitation token",
            secret: false,
            read_only: false,
            value: String::new(),
        },
        FormField {
            label: "Email",
            secret: false,
            read_only: false,
            value: String::new(),
        },
        FormField {
            label: "Password",
            secret: true,
            read_only: false,
            value: String::new(),
        },
        FormField {
            label: "Display name",
            secret: false,
            read_only: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn posture_fields(active_account: Option<AccountId>) -> Vec<FormField> {
    vec![
        FormField {
            label: "Account target",
            secret: false,
            read_only: true,
            value: active_account.map_or_else(String::new, |id| id.to_string()),
        },
        FormField {
            label: "Posture",
            secret: false,
            read_only: false,
            value: "hosted".to_owned(),
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

pub(crate) fn posture_outcome(
    current: Option<SetDeploymentPostureResponse>,
    supported: &[SupportedDeploymentPosture],
) -> OutcomeView {
    let mut lines = Vec::new();
    lines.push("supported postures:".to_owned());
    for entry in supported {
        lines.push(format!(
            "- {} | available: {} | unavailable: {}",
            entry.posture.as_wire_value(),
            format_caps(&entry.capability_summary.available),
            format_unavailable_caps(&entry.capability_summary.unavailable),
        ));
    }
    if let Some(current) = current {
        lines.push(String::new());
        lines.push(format!("current scope: {}", describe_scope(current.scope)));
        lines.push(format!(
            "current posture: {}",
            current.posture.as_wire_value()
        ));
        lines.push(format!(
            "available: {}",
            format_caps(&current.capability_summary.available)
        ));
        lines.push(format!(
            "unavailable: {}",
            format_unavailable_caps(&current.capability_summary.unavailable)
        ));
        lines.push(format!("audit reference: {}", current.audit_reference));
    } else {
        lines.push(String::new());
        lines.push("current posture: not set".to_owned());
    }
    OutcomeView {
        title: "Deployment posture",
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

pub(crate) fn render_posture_error(err: &SetDeploymentPostureError) -> String {
    if let Some(failure) = err.contract_failure() {
        let body = failure.render();
        return render_posture_failure(&body);
    }
    let body = tanren_contract::DeploymentPostureFailureReason::InternalError.render(None);
    render_posture_failure(&body)
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

pub(crate) fn parse_posture(
    state: &FormState,
    active_account: AccountId,
) -> Result<SetDeploymentPostureRequest, String> {
    let raw_posture = state.value(1);
    let posture = DeploymentPosture::from_str(raw_posture).map_err(|failure| {
        let body = failure.render();
        format!("{}: {}", body.code, body.summary)
    })?;
    let request = SetDeploymentPostureRequest {
        scope: tanren_contract::DeploymentPostureScope::Account {
            account_id: active_account,
        },
        posture,
    };
    Ok(request)
}

fn render_posture_failure(body: &tanren_contract::DeploymentPostureFailureBody) -> String {
    format!("{}: {}", body.code, body.summary)
}

fn format_caps(caps: &[DeploymentPostureCapability]) -> String {
    if caps.is_empty() {
        return "none".to_owned();
    }
    let values: Vec<&str> = caps.iter().copied().map(capability_name).collect();
    values.join(", ")
}

fn format_unavailable_caps(caps: &[DeploymentPostureUnavailableCapability]) -> String {
    if caps.is_empty() {
        return "none".to_owned();
    }
    let values: Vec<String> = caps
        .iter()
        .map(|entry| {
            format!(
                "{} ({})",
                capability_name(entry.capability),
                entry.reason.summary()
            )
        })
        .collect();
    values.join(", ")
}

const fn capability_name(cap: DeploymentPostureCapability) -> &'static str {
    cap.as_wire_value()
}

const fn describe_scope(scope: tanren_contract::DeploymentPostureScope) -> &'static str {
    match scope {
        tanren_contract::DeploymentPostureScope::Account { .. } => "account",
        tanren_contract::DeploymentPostureScope::Project { .. } => "project",
        tanren_contract::DeploymentPostureScope::Installation { .. } => "installation",
    }
}
