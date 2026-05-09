use anyhow::{Context, Result};
use tanren_identity_policy::{
    AccountId, OrgId, PermissionName, PermissionScope, PrincipalRef, ProjectId, RoleId, RoleScope,
};
use uuid::Uuid;

use crate::role::{PrincipalKind, ScopeKind};

pub(super) fn parse_permissions(raw: Vec<String>) -> Result<Vec<PermissionName>> {
    raw.into_iter()
        .map(|value| {
            PermissionName::parse(&value)
                .with_context(|| format!("parse --permission `{value}` as permission name"))
        })
        .collect()
}

pub(super) fn parse_role_scope(kind: ScopeKind, id: &str, field_name: &str) -> Result<RoleScope> {
    let uuid = parse_uuid(id, field_name)?;
    Ok(match kind {
        ScopeKind::Account => RoleScope::Account {
            account_id: AccountId::from(uuid),
        },
        ScopeKind::Organization => RoleScope::Organization {
            org_id: OrgId::from(uuid),
        },
        ScopeKind::Project => RoleScope::Project {
            project_id: ProjectId::from(uuid),
        },
    })
}

pub(super) fn parse_permission_scope(
    kind: ScopeKind,
    id: &str,
    field_name: &str,
) -> Result<PermissionScope> {
    let uuid = parse_uuid(id, field_name)?;
    Ok(match kind {
        ScopeKind::Account => PermissionScope::Account {
            account_id: AccountId::from(uuid),
        },
        ScopeKind::Organization => PermissionScope::Organization {
            org_id: OrgId::from(uuid),
        },
        ScopeKind::Project => PermissionScope::Project {
            project_id: ProjectId::from(uuid),
        },
    })
}

pub(super) fn parse_principal(kind: PrincipalKind, id: &str) -> Result<PrincipalRef> {
    let uuid = parse_uuid(id, "--principal-id")?;
    Ok(match kind {
        PrincipalKind::Account => PrincipalRef::Account {
            account_id: AccountId::from(uuid),
        },
        PrincipalKind::Role => PrincipalRef::Role {
            role_id: RoleId::from(uuid),
        },
    })
}

pub(super) fn parse_role_id(raw: &str) -> Result<RoleId> {
    let uuid = parse_uuid(raw, "--role-id")?;
    Ok(RoleId::from(uuid))
}

fn parse_uuid(raw: &str, field_name: &str) -> Result<Uuid> {
    Uuid::parse_str(raw).with_context(|| format!("parse {field_name} as uuid"))
}
