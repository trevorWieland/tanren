use std::sync::OnceLock;

use regex::Regex;
use tanren_identity_policy::{
    AccountId, OrgId, PermissionScope, PrincipalRef, RoleScope, ScopedRole,
};
use uuid::Uuid;

use super::tui_driver::TuiTranscript;
use super::{RoleHarnessError, RoleHarnessResult};

fn extract_uuid(raw: &str, field: &str) -> Result<Uuid, String> {
    static UUID_RE: OnceLock<Regex> = OnceLock::new();
    let re = UUID_RE.get_or_init(|| {
        Regex::new(r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}")
            .expect("uuid regex must compile")
    });
    let candidate = re
        .find(raw)
        .map(|m| m.as_str())
        .ok_or_else(|| format!("parse {field} from tui output: no uuid in `{raw}`"))?;
    Uuid::parse_str(candidate).map_err(|e| format!("parse {field} from tui output: {e}"))
}

pub(crate) fn parse_role_from_transcript(
    transcript: &TuiTranscript,
) -> RoleHarnessResult<ScopedRole> {
    let role_id_raw = transcript
        .line_value("role_id")
        .ok_or_else(|| RoleHarnessError::Transport("missing role_id in tui output".to_owned()))?;
    let scope_raw = transcript
        .line_value("scope")
        .ok_or_else(|| RoleHarnessError::Transport("missing scope in tui output".to_owned()))?;

    let role_uuid = extract_uuid(&role_id_raw, "role_id").map_err(RoleHarnessError::Transport)?;
    let (scope_kind, scope_id) = scope_raw.split_once(':').ok_or_else(|| {
        RoleHarnessError::Transport(format!(
            "parse scope from tui output: invalid `{scope_raw}`"
        ))
    })?;
    let scope_uuid = extract_uuid(scope_id, "scope").map_err(RoleHarnessError::Transport)?;

    let scope = match scope_kind {
        "account" => RoleScope::Account {
            account_id: AccountId::from(scope_uuid),
        },
        "organization" => RoleScope::Organization {
            org_id: OrgId::new(scope_uuid),
        },
        "project" => RoleScope::Project {
            project_id: tanren_identity_policy::ProjectId::new(scope_uuid),
        },
        _ => {
            return Err(RoleHarnessError::Transport(format!(
                "parse scope kind from tui output: invalid `{scope_kind}`"
            )));
        }
    };

    Ok(ScopedRole {
        role_id: tanren_identity_policy::RoleId::new(role_uuid),
        scope,
    })
}

#[must_use]
pub(crate) fn role_scope_kind_label(scope: RoleScope) -> &'static str {
    match scope {
        RoleScope::Account { .. } => "account",
        RoleScope::Organization { .. } => "organization",
        RoleScope::Project { .. } => "project",
    }
}

#[must_use]
pub(crate) fn role_scope_id(scope: RoleScope) -> Uuid {
    match scope {
        RoleScope::Account { account_id } => account_id.as_uuid(),
        RoleScope::Organization { org_id } => org_id.as_uuid(),
        RoleScope::Project { project_id } => project_id.as_uuid(),
    }
}

#[must_use]
pub(crate) fn permission_scope_kind_label(scope: PermissionScope) -> &'static str {
    match scope {
        PermissionScope::Account { .. } => "account",
        PermissionScope::Organization { .. } => "organization",
        PermissionScope::Project { .. } => "project",
    }
}

#[must_use]
pub(crate) fn permission_scope_id(scope: PermissionScope) -> Uuid {
    match scope {
        PermissionScope::Account { account_id } => account_id.as_uuid(),
        PermissionScope::Organization { org_id } => org_id.as_uuid(),
        PermissionScope::Project { project_id } => project_id.as_uuid(),
    }
}

#[must_use]
pub(crate) fn principal_parts(principal: PrincipalRef) -> (&'static str, Uuid) {
    match principal {
        PrincipalRef::Account { account_id } => ("account", account_id.as_uuid()),
        PrincipalRef::Role { role_id } => ("role", role_id.as_uuid()),
    }
}
