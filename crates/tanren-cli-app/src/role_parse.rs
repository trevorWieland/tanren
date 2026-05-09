use anyhow::{Context, Result};
use tanren_contract::{
    parse_permission_scope_field, parse_principal_field, parse_role_id_field,
    parse_role_scope_field,
};
use tanren_identity_policy::{PermissionName, PermissionScope, PrincipalRef, RoleId, RoleScope};

use crate::role::{PrincipalKind, ScopeKind};

pub(super) fn parse_permissions(raw: Vec<String>) -> Result<Vec<PermissionName>> {
    raw.into_iter()
        .map(|value| {
            PermissionName::parse(&value)
                .with_context(|| format!("parse --permission `{value}` as permission name"))
        })
        .collect()
}

pub(super) fn parse_role_scope(
    kind: ScopeKind,
    id: &str,
    field_name: &'static str,
) -> Result<RoleScope> {
    parse_role_scope_field(kind.as_str(), id, "--scope-kind", field_name)
        .with_context(|| format!("parse {field_name} as role scope"))
}

pub(super) fn parse_permission_scope(
    kind: ScopeKind,
    id: &str,
    field_name: &'static str,
) -> Result<PermissionScope> {
    parse_permission_scope_field(kind.as_str(), id, "--scope-kind", field_name)
        .with_context(|| format!("parse {field_name} as permission scope"))
}

pub(super) fn parse_principal(kind: PrincipalKind, id: &str) -> Result<PrincipalRef> {
    parse_principal_field(kind.as_str(), id, "--principal-kind", "--principal-id")
        .context("parse principal")
}

pub(super) fn parse_role_id(raw: &str) -> Result<RoleId> {
    parse_role_id_field(raw, "--role-id").context("parse role id")
}
