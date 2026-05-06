//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use chrono::{DateTime, NaiveDateTime, Utc};
use secrecy::SecretString;
use tanren_app_services::AppServiceError;
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason,
    CreateOrgInvitationRequest, CreateOrgInvitationResponse, InvitationStatus,
    ListOrgInvitationsResponse, OrgInvitationView, RevokeOrgInvitationRequest,
    RevokeOrgInvitationResponse, SignInRequest, SignInResponse, SignUpRequest, SignUpResponse,
};
use tanren_identity_policy::{
    AccountId, Email, Identifier, InvitationToken, OrgId, OrganizationPermission, ValidationError,
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

pub(crate) fn create_invitation_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Account ID (admin)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Org ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Recipient identifier",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Permissions (comma-separated)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Expires at (RFC 3339 or YYYY-MM-DD HH:MM)",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn list_invitations_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Account ID (admin)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Org ID",
            secret: false,
            value: String::new(),
        },
    ]
}

pub(crate) fn revoke_invitation_fields() -> Vec<FormField> {
    vec![
        FormField {
            label: "Account ID (admin)",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Org ID",
            secret: false,
            value: String::new(),
        },
        FormField {
            label: "Invitation token",
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

pub(crate) fn create_invitation_outcome(response: &CreateOrgInvitationResponse) -> OutcomeView {
    let inv = &response.invitation;
    OutcomeView {
        title: "Invitation created",
        lines: vec![
            format!("token: {}", inv.token),
            format!("org_id: {}", inv.org_id),
            format!("recipient: {}", inv.recipient_identifier),
            format!("permissions: {}", format_permissions(&inv.permissions)),
            format!("status: {}", format_status(inv.status)),
            format!("expires_at: {}", inv.expires_at),
        ],
    }
}

pub(crate) fn list_invitations_outcome(response: &ListOrgInvitationsResponse) -> OutcomeView {
    let mut lines: Vec<String> = Vec::new();
    if response.invitations.is_empty() {
        lines.push("No invitations found.".to_owned());
    } else {
        for inv in &response.invitations {
            lines.push(format_invitation_summary(inv));
        }
    }
    OutcomeView {
        title: "Organization invitations",
        lines,
    }
}

pub(crate) fn revoke_invitation_outcome(response: &RevokeOrgInvitationResponse) -> OutcomeView {
    let inv = &response.invitation;
    OutcomeView {
        title: "Invitation revoked",
        lines: vec![
            format!("token: {}", inv.token),
            format!("org_id: {}", inv.org_id),
            format!("recipient: {}", inv.recipient_identifier),
            format!("status: {}", format_status(inv.status)),
        ],
    }
}

fn format_invitation_summary(inv: &OrgInvitationView) -> String {
    format!(
        "token={} recipient={} permissions=[{}] status={} expires={}",
        inv.token,
        inv.recipient_identifier,
        format_permissions(&inv.permissions),
        format_status(inv.status),
        inv.expires_at,
    )
}

fn format_permissions(permissions: &[OrganizationPermission]) -> String {
    permissions
        .iter()
        .map(OrganizationPermission::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_status(status: InvitationStatus) -> &'static str {
    match status {
        InvitationStatus::Pending => "pending",
        InvitationStatus::Accepted => "accepted",
        InvitationStatus::Revoked => "revoked",
        _ => "unknown",
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

pub(crate) struct CreateInvitationInputs {
    pub(crate) caller_account_id: AccountId,
    pub(crate) caller_org_context: Option<OrgId>,
    pub(crate) request: CreateOrgInvitationRequest,
}

pub(crate) fn parse_create_invitation(state: &FormState) -> Result<CreateInvitationInputs, String> {
    let account_uuid =
        Uuid::parse_str(state.value(0)).map_err(|e| format!("invalid account id: {e}"))?;
    let caller_account_id = AccountId::new(account_uuid);
    let org_uuid = Uuid::parse_str(state.value(1)).map_err(|e| format!("invalid org id: {e}"))?;
    let org_id = OrgId::new(org_uuid);
    let recipient_identifier =
        Identifier::parse(state.value(2)).map_err(|e| validation_message(&e))?;
    let permissions = parse_permissions(state.value(3))?;
    let expires_at = parse_datetime(state.value(4))?;
    Ok(CreateInvitationInputs {
        caller_account_id,
        caller_org_context: Some(org_id),
        request: CreateOrgInvitationRequest {
            org_id,
            recipient_identifier,
            permissions,
            expires_at,
        },
    })
}

pub(crate) struct ListInvitationInputs {
    pub(crate) caller_account_id: AccountId,
    pub(crate) org_id: OrgId,
}

pub(crate) fn parse_list_invitation_inputs(
    state: &FormState,
) -> Result<ListInvitationInputs, String> {
    let account_uuid =
        Uuid::parse_str(state.value(0)).map_err(|e| format!("invalid account id: {e}"))?;
    let caller_account_id = AccountId::new(account_uuid);
    let org_uuid = Uuid::parse_str(state.value(1)).map_err(|e| format!("invalid org id: {e}"))?;
    let org_id = OrgId::new(org_uuid);
    Ok(ListInvitationInputs {
        caller_account_id,
        org_id,
    })
}

pub(crate) struct RevokeInvitationInputs {
    pub(crate) caller_account_id: AccountId,
    pub(crate) caller_org_context: Option<OrgId>,
    pub(crate) request: RevokeOrgInvitationRequest,
}

pub(crate) fn parse_revoke_invitation(state: &FormState) -> Result<RevokeInvitationInputs, String> {
    let account_uuid =
        Uuid::parse_str(state.value(0)).map_err(|e| format!("invalid account id: {e}"))?;
    let caller_account_id = AccountId::new(account_uuid);
    let org_uuid = Uuid::parse_str(state.value(1)).map_err(|e| format!("invalid org id: {e}"))?;
    let org_id = OrgId::new(org_uuid);
    let token = InvitationToken::parse(state.value(2)).map_err(|e| validation_message(&e))?;
    Ok(RevokeInvitationInputs {
        caller_account_id,
        caller_org_context: Some(org_id),
        request: RevokeOrgInvitationRequest { org_id, token },
    })
}

fn parse_permissions(raw: &str) -> Result<Vec<OrganizationPermission>, String> {
    if raw.trim().is_empty() {
        return Ok(vec![]);
    }
    raw.split(',')
        .map(|s| {
            OrganizationPermission::parse(s.trim()).map_err(|e| format!("invalid permission: {e}"))
        })
        .collect()
}

fn parse_datetime(raw: &str) -> Result<DateTime<Utc>, String> {
    let trimmed = raw.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(trimmed) {
        return Ok(dt.to_utc());
    }
    NaiveDateTime::parse_from_str(trimmed, "%Y-%m-%d %H:%M")
        .map(|naive| naive.and_utc())
        .map_err(|e| format!("invalid datetime (use RFC 3339 or YYYY-MM-DD HH:MM): {e}"))
}
