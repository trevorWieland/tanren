//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep this file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_app_services::{ApplyError, PreviewError};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, AssetAction,
    SignInRequest, SignInResponse, SignUpRequest, SignUpResponse, UpgradePreviewResponse,
};
use tanren_identity_policy::{Email, InvitationToken, ValidationError};

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

pub(crate) fn upgrade_root_fields() -> Vec<FormField> {
    vec![FormField {
        label: "Root path",
        secret: false,
        value: String::new(),
    }]
}

pub(crate) fn upgrade_apply_outcome(response: &UpgradePreviewResponse) -> OutcomeView {
    OutcomeView {
        title: "Upgrade applied",
        lines: upgrade_summary_lines(response),
    }
}

pub(crate) fn upgrade_summary_lines(response: &UpgradePreviewResponse) -> Vec<String> {
    let mut lines = vec![format!(
        "{} → {}",
        response.source_version, response.target_version
    )];
    for action in &response.actions {
        lines.push(format_action_label(action));
    }
    for concern in &response.concerns {
        lines.push(format!("\u{26a0} {:?}: {}", concern.kind, concern.detail));
    }
    for path in &response.preserved_user_paths {
        lines.push(format!("preserved: {}", path.display()));
    }
    lines
}

fn format_action_label(action: &AssetAction) -> String {
    match action {
        AssetAction::Create { path, .. } => format!("create: {}", path.display()),
        AssetAction::Update { path, .. } => format!("update: {}", path.display()),
        AssetAction::Remove { path, .. } => format!("remove: {}", path.display()),
        AssetAction::Preserve { path, .. } => format!("preserve: {}", path.display()),
    }
}

pub(crate) fn render_preview_error(err: &PreviewError) -> String {
    format!("preview_failed: {err}")
}

pub(crate) fn render_apply_error(err: ApplyError) -> String {
    match err {
        ApplyError::Preview(preview_err) => render_preview_error(&preview_err),
        _ => format!("apply_failed: {err}"),
    }
}
