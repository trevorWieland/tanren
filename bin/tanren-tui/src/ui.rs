//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, SignInRequest,
    SignInResponse, SignUpRequest, SignUpResponse,
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
    let password = SecretString::from(state.value(1).to_owned());
    let display_name = state.value(2).to_owned();
    // PR 7 sources the email from the invitation row; for PR 3 the TUI
    // lacks an email field on the accept-invitation form, so we synthesise
    // a placeholder derived from the invitation token. The synthesised
    // email is unique per invitation token (the token must be ≥ 16 bytes
    // so the resulting local-part is non-empty).
    let token_str = invitation_token.as_str().to_owned();
    let synthesised = format!("{token_str}@invitation.tanren");
    let email = Email::parse(&synthesised).map_err(|e| validation_message(&e))?;
    Ok(AcceptInvitationRequest {
        invitation_token,
        email,
        password,
        display_name,
    })
}
