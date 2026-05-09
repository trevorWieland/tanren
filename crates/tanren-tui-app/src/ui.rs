//! Form factories, outcome adapters, and error-message helpers for
//! the TUI. Split out of `main.rs` to keep that file under the
//! workspace 500-line budget.

use secrecy::SecretString;
use tanren_app_services::{AppServiceError, RoleServiceError};
use tanren_contract::{
    AcceptInvitationRequest, AcceptInvitationResponse, AccountFailureReason, ApplyRoleRequest,
    ApplyRoleResponse, CreateRoleRequest, CreateRoleResponse, EditRoleRequest, EditRoleResponse,
    PermissionCheckRequest, PermissionCheckResponse, SignInRequest, SignInResponse, SignUpRequest,
    SignUpResponse, format_permission_scope, format_role_scope, parse_permission_scope_field,
    parse_principal_field, parse_role_id_field, parse_role_scope_field,
};
use tanren_identity_policy::{
    Email, InvitationToken, PermissionName, PermissionScope, PrincipalRef, RoleId, RoleName,
    RoleScope, ScopedRole, ValidationError,
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

pub(crate) fn create_role_fields() -> Vec<FormField> {
    vec![
        field("Scope kind (account|organization|project)"),
        field("Scope id (uuid)"),
        field("Role name"),
        field("Permissions (comma-separated)"),
    ]
}

pub(crate) fn edit_role_fields() -> Vec<FormField> {
    vec![
        field("Role id (uuid)"),
        field("Scope kind (account|organization|project)"),
        field("Scope id (uuid)"),
        field("Role name"),
        field("Permissions (comma-separated)"),
    ]
}

pub(crate) fn delete_role_fields() -> Vec<FormField> {
    vec![
        field("Role id (uuid)"),
        field("Scope kind (account|organization|project)"),
        field("Scope id (uuid)"),
    ]
}

pub(crate) fn apply_role_fields() -> Vec<FormField> {
    vec![
        field("Role id (uuid)"),
        field("Role scope kind (account|organization|project)"),
        field("Role scope id (uuid)"),
        field("Principal kind (account|role)"),
        field("Principal id (uuid)"),
        field("Grant scope kind (account|organization|project)"),
        field("Grant scope id (uuid)"),
    ]
}

pub(crate) fn permission_check_fields() -> Vec<FormField> {
    vec![
        field("Principal kind (account|role)"),
        field("Principal id (uuid)"),
        field("Permission"),
        field("Scope kind (account|organization|project)"),
        field("Scope id (uuid)"),
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

pub(crate) fn create_role_outcome(response: &CreateRoleResponse) -> OutcomeView {
    role_outcome("Role created", &response.role)
}

pub(crate) fn edit_role_outcome(response: &EditRoleResponse) -> OutcomeView {
    role_outcome("Role updated", &response.role)
}

pub(crate) fn delete_role_outcome(role: ScopedRole) -> OutcomeView {
    OutcomeView {
        title: "Role deleted",
        lines: vec![
            format!("role_id: {}", role.role_id),
            format!("scope: {}", format_role_scope(role.scope)),
        ],
    }
}

pub(crate) fn apply_role_outcome(response: &ApplyRoleResponse) -> OutcomeView {
    OutcomeView {
        title: "Role applied",
        lines: vec![
            format!("role_id: {}", response.role.role_id),
            format!("scope: {}", format_role_scope(response.role.scope)),
            format!("grants_created: {}", response.grants.len()),
        ],
    }
}

pub(crate) fn permission_check_outcome(response: &PermissionCheckResponse) -> OutcomeView {
    OutcomeView {
        title: "Permission checked",
        lines: vec![
            format!("allowed: {}", response.allowed),
            format!("permission: {}", response.permission),
            format!("scope: {}", format_permission_scope(response.scope)),
            format!("matching_grants: {}", response.matching_grant_ids.len()),
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

pub(crate) fn render_role_error(err: RoleServiceError) -> String {
    match err {
        RoleServiceError::Role(reason) => format!("{}: {}", reason.code(), reason.summary()),
        RoleServiceError::InvalidInput(message) => format!("validation_failed: {message}"),
        RoleServiceError::Store(err) => {
            tracing::error!(target: "tanren_tui", error = %err, "role store failure");
            "internal_error: Tanren encountered an internal error.".to_owned()
        }
        other => {
            tracing::error!(target: "tanren_tui", error = ?other, "unexpected role failure");
            "internal_error: Tanren encountered an internal error.".to_owned()
        }
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

pub(crate) fn parse_create_role(state: &FormState) -> Result<CreateRoleRequest, String> {
    Ok(CreateRoleRequest {
        scope: parse_role_scope(state.value(0), state.value(1))?,
        name: RoleName::parse(state.value(2)).map_err(|e| format!("validation_failed: {e}"))?,
        permissions: parse_permissions(state.value(3))?,
    })
}

pub(crate) fn parse_edit_role(state: &FormState) -> Result<EditRoleRequest, String> {
    Ok(EditRoleRequest {
        role: ScopedRole {
            role_id: parse_role_id(state.value(0), "role id")?,
            scope: parse_role_scope(state.value(1), state.value(2))?,
        },
        name: RoleName::parse(state.value(3)).map_err(|e| format!("validation_failed: {e}"))?,
        permissions: parse_permissions(state.value(4))?,
    })
}

pub(crate) fn parse_delete_role(state: &FormState) -> Result<ScopedRole, String> {
    Ok(ScopedRole {
        role_id: parse_role_id(state.value(0), "role id")?,
        scope: parse_role_scope(state.value(1), state.value(2))?,
    })
}

pub(crate) fn parse_apply_role(state: &FormState) -> Result<ApplyRoleRequest, String> {
    Ok(ApplyRoleRequest {
        role: ScopedRole {
            role_id: parse_role_id(state.value(0), "role id")?,
            scope: parse_role_scope(state.value(1), state.value(2))?,
        },
        principal: parse_principal(state.value(3), state.value(4))?,
        grant_scope: parse_permission_scope(state.value(5), state.value(6))?,
    })
}

pub(crate) fn parse_permission_check(state: &FormState) -> Result<PermissionCheckRequest, String> {
    Ok(PermissionCheckRequest {
        principal: parse_principal(state.value(0), state.value(1))?,
        permission: PermissionName::parse(state.value(2))
            .map_err(|e| format!("validation_failed: {e}"))?,
        scope: parse_permission_scope(state.value(3), state.value(4))?,
    })
}

fn field(label: &'static str) -> FormField {
    FormField {
        label,
        secret: false,
        value: String::new(),
    }
}

fn parse_role_scope(kind: &str, id: &str) -> Result<RoleScope, String> {
    parse_role_scope_field(kind, id, "scope kind", "scope id")
        .map_err(|err| validation_from_adapter(&err))
}

fn parse_permission_scope(kind: &str, id: &str) -> Result<PermissionScope, String> {
    parse_permission_scope_field(kind, id, "scope kind", "scope id")
        .map_err(|err| validation_from_adapter(&err))
}

fn parse_principal(kind: &str, id: &str) -> Result<PrincipalRef, String> {
    parse_principal_field(kind, id, "principal kind", "principal id")
        .map_err(|err| validation_from_adapter(&err))
}

fn parse_permissions(raw: &str) -> Result<Vec<PermissionName>, String> {
    let mut out = Vec::new();
    for value in raw.split(',').map(str::trim).filter(|v| !v.is_empty()) {
        out.push(PermissionName::parse(value).map_err(|e| format!("validation_failed: {e}"))?);
    }
    if out.is_empty() {
        return Err("validation_failed: at least one permission is required".to_owned());
    }
    Ok(out)
}

fn parse_role_id(raw: &str, label: &'static str) -> Result<RoleId, String> {
    parse_role_id_field(raw, label).map_err(|err| validation_from_adapter(&err))
}

fn role_outcome(title: &'static str, role: &tanren_contract::RoleTemplateView) -> OutcomeView {
    OutcomeView {
        title,
        lines: vec![
            format!("role_id: {}", role.id),
            format!("scope: {}", format_role_scope(role.scope)),
            format!("name: {}", role.name),
            format!(
                "permissions: {}",
                role.permissions
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        ],
    }
}

fn validation_from_adapter(err: &tanren_contract::RoleAdapterError) -> String {
    format!("validation_failed: {err}")
}
