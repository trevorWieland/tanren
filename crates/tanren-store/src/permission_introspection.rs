//! Read-model query for self-permission introspection.

use crate::{
    MyOrganizationPermissionsRecord, MyPermissionRecord, MyPermissionsPage, MyPermissionsRecord,
    MyProjectPermissionsRecord, PermissionConstraintRecord, StoreError,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use tanren_identity_policy::{
    AccountId, OrgId, PermissionEffectiveState, PermissionGrantSource, PermissionName,
    PolicyConstraintReason, ProjectId, RoleTemplateName,
};

pub(crate) async fn load_my_permissions(
    conn: &DatabaseConnection,
    account_id: AccountId,
    page: MyPermissionsPage,
) -> Result<MyPermissionsRecord, StoreError> {
    let rows = conn
        .query_all(introspection_query(
            conn.get_database_backend(),
            account_id,
            page.limit,
        ))
        .await?;

    let mut organizations: Vec<MyOrganizationPermissionsRecord> = Vec::new();
    let mut projects: Vec<MyProjectPermissionsRecord> = Vec::new();

    for row in rows {
        let org_id: Option<uuid::Uuid> = row.try_get("", "org_id")?;
        let project_id: Option<uuid::Uuid> = row.try_get("", "project_id")?;
        let permission_name: String = row.try_get("", "permission_name")?;
        let role_template_name: Option<String> = row.try_get("", "role_template_name")?;
        let constraint_reason: Option<String> = row.try_get("", "constraint_reason")?;
        let constraint_is_project_policy: Option<bool> =
            row.try_get("", "constraint_is_project_policy")?;

        let policy_constraint =
            parse_policy_constraint(constraint_reason, constraint_is_project_policy)?;
        let effective_state = if policy_constraint.is_some() {
            PermissionEffectiveState::Constrained
        } else {
            PermissionEffectiveState::Granted
        };
        let permission = MyPermissionRecord {
            permission: parse_permission_name(&permission_name)?,
            effective_state,
            grant_source: parse_grant_source(role_template_name)?,
            policy_constraint,
        };
        match (org_id.map(OrgId::new), project_id.map(ProjectId::new)) {
            (Some(scope_org_id), None) => {
                append_organization_permission(&mut organizations, scope_org_id, permission);
            }
            (None, Some(scope_project_id)) => {
                append_project_permission(&mut projects, scope_project_id, permission);
            }
            _ => {
                return Err(StoreError::Invariant {
                    entity: "permission_grants",
                    detail: "each grant must have exactly one scope (org or project)",
                });
            }
        }
    }

    Ok(MyPermissionsRecord {
        organizations,
        projects,
    })
}

fn append_organization_permission(
    organizations: &mut Vec<MyOrganizationPermissionsRecord>,
    org_id: OrgId,
    permission: MyPermissionRecord,
) {
    if let Some(last) = organizations.last_mut()
        && last.org_id == org_id
    {
        last.permissions.push(permission);
        return;
    }

    organizations.push(MyOrganizationPermissionsRecord {
        org_id,
        permissions: vec![permission],
    });
}

fn append_project_permission(
    projects: &mut Vec<MyProjectPermissionsRecord>,
    project_id: ProjectId,
    permission: MyPermissionRecord,
) {
    if let Some(last) = projects.last_mut()
        && last.project_id == project_id
    {
        last.permissions.push(permission);
        return;
    }

    projects.push(MyProjectPermissionsRecord {
        project_id,
        permissions: vec![permission],
    });
}

fn introspection_query(backend: DbBackend, account_id: AccountId, limit: u16) -> Statement {
    let sql = match backend {
        DbBackend::Postgres => INTROSPECTION_QUERY_POSTGRES,
        DbBackend::Sqlite | DbBackend::MySql => INTROSPECTION_QUERY_SQLITE,
    };
    Statement::from_sql_and_values(
        backend,
        sql,
        [account_id.as_uuid().into(), i64::from(limit).into()],
    )
}

fn parse_grant_source(
    role_template_name: Option<String>,
) -> Result<PermissionGrantSource, StoreError> {
    match role_template_name {
        Some(role_template) => Ok(PermissionGrantSource::RoleTemplate {
            role_template: parse_role_template_name(&role_template)?,
        }),
        None => Ok(PermissionGrantSource::Direct),
    }
}

fn parse_policy_constraint(
    constraint_reason: Option<String>,
    constraint_is_project_policy: Option<bool>,
) -> Result<Option<PermissionConstraintRecord>, StoreError> {
    match (constraint_reason, constraint_is_project_policy) {
        (Some(reason), Some(is_project_policy)) => Ok(Some(PermissionConstraintRecord {
            reason: parse_policy_constraint_reason(&reason)?,
            source: if is_project_policy {
                tanren_identity_policy::PolicyConstraintSource::ProjectPolicy
            } else {
                tanren_identity_policy::PolicyConstraintSource::OrganizationPolicy
            },
        })),
        (None, None) => Ok(None),
        _ => Err(StoreError::Invariant {
            entity: "permission_constraints",
            detail: "constraint reason/source columns must be both null or both set",
        }),
    }
}

fn parse_permission_name(raw: &str) -> Result<PermissionName, StoreError> {
    PermissionName::parse(raw).map_err(|cause| StoreError::DataInvariant {
        column: "permission_grants.permission_name",
        cause,
    })
}

fn parse_role_template_name(raw: &str) -> Result<RoleTemplateName, StoreError> {
    RoleTemplateName::parse(raw).map_err(|cause| StoreError::DataInvariant {
        column: "permission_grants.role_template_name",
        cause,
    })
}

fn parse_policy_constraint_reason(raw: &str) -> Result<PolicyConstraintReason, StoreError> {
    PolicyConstraintReason::parse(raw).map_err(|cause| StoreError::DataInvariant {
        column: "permission_constraints.reason",
        cause,
    })
}

const INTROSPECTION_QUERY_POSTGRES: &str = r"
SELECT
  pg.org_id AS org_id,
  pg.project_id AS project_id,
  pg.permission_name,
  pg.role_template_name,
  pc.reason AS constraint_reason,
  pc.is_project_policy AS constraint_is_project_policy
FROM permission_grants pg
LEFT JOIN permission_constraints pc ON pc.grant_id = pg.id
WHERE pg.account_id = $1
ORDER BY
  CASE
    WHEN pg.org_id IS NOT NULL AND pg.project_id IS NULL THEN 0
    WHEN pg.org_id IS NULL AND pg.project_id IS NOT NULL THEN 1
    ELSE 2
  END,
  COALESCE(pg.org_id, pg.project_id),
  pg.permission_name,
  CASE WHEN pg.role_template_name IS NULL THEN 0 ELSE 1 END,
  COALESCE(pg.role_template_name, ''),
  pg.id
LIMIT $2
";

const INTROSPECTION_QUERY_SQLITE: &str = r"
SELECT
  pg.org_id AS org_id,
  pg.project_id AS project_id,
  pg.permission_name,
  pg.role_template_name,
  pc.reason AS constraint_reason,
  pc.is_project_policy AS constraint_is_project_policy
FROM permission_grants pg
LEFT JOIN permission_constraints pc ON pc.grant_id = pg.id
WHERE pg.account_id = ?
ORDER BY
  CASE
    WHEN pg.org_id IS NOT NULL AND pg.project_id IS NULL THEN 0
    WHEN pg.org_id IS NULL AND pg.project_id IS NOT NULL THEN 1
    ELSE 2
  END,
  COALESCE(pg.org_id, pg.project_id),
  pg.permission_name,
  CASE WHEN pg.role_template_name IS NULL THEN 0 ELSE 1 END,
  COALESCE(pg.role_template_name, ''),
  pg.id
LIMIT ?
";
