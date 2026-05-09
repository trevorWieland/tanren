//! Read-model query for self-permission introspection.

use crate::{
    MyOrganizationPermissionsRecord, MyPermissionRecord, MyPermissionsCursor, MyPermissionsPage,
    MyPermissionsRecord, MyPermissionsScopeKind, MyProjectPermissionsRecord,
    PermissionConstraintId, PermissionConstraintRecord, PermissionGrantId, StoreError,
};
use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use tanren_identity_policy::{
    AccountId, OrgId, PermissionEffectiveState, PermissionGrantSource, PermissionName,
    PolicyConstraintReason, ProjectId, RoleTemplateName,
};
use uuid::Uuid;

pub(crate) async fn load_my_permissions(
    conn: &DatabaseConnection,
    account_id: AccountId,
    page: MyPermissionsPage,
) -> Result<MyPermissionsRecord, StoreError> {
    let generated_at = Utc::now();
    let limit_plus_one = page.limit.saturating_add(1);
    let query = introspection_query(
        conn.get_database_backend(),
        account_id,
        page.cursor.as_ref(),
        limit_plus_one,
    )?;
    let rows = conn.query_all(query).await?;

    let mut parsed_rows: Vec<ParsedPermissionRow> = rows
        .iter()
        .map(parse_permission_row)
        .collect::<Result<_, _>>()?;

    let has_next_page = parsed_rows.len() > usize::from(page.limit);
    if has_next_page {
        parsed_rows.truncate(usize::from(page.limit));
    }

    let mut organizations: Vec<MyOrganizationPermissionsRecord> = Vec::new();
    let mut projects: Vec<MyProjectPermissionsRecord> = Vec::new();

    for row in &parsed_rows {
        match row.scope {
            Scope::Organization(scope_org_id) => append_organization_permission(
                &mut organizations,
                scope_org_id,
                row.permission.clone(),
            ),
            Scope::Project(scope_project_id) => {
                append_project_permission(&mut projects, scope_project_id, row.permission.clone());
            }
        }
    }

    Ok(MyPermissionsRecord {
        next_cursor: if has_next_page {
            parsed_rows.last().map(|row| row.cursor.clone())
        } else {
            None
        },
        freshness: crate::MyPermissionsFreshnessRecord {
            projection: "permission_introspection_permission_grants_v1".to_owned(),
            checkpoint: parsed_rows
                .last()
                .map(|row| format!("permission_grants:{}", row.cursor.grant_id.as_uuid())),
            generated_at,
            is_stale: false,
        },
        organizations,
        projects,
    })
}

fn parse_permission_row(row: &QueryResult) -> Result<ParsedPermissionRow, StoreError> {
    let org_id: Option<Uuid> = row.try_get("", "org_id")?;
    let project_id: Option<Uuid> = row.try_get("", "project_id")?;
    let permission_name: String = row.try_get("", "permission_name")?;
    let role_template_name: Option<String> = row.try_get("", "role_template_name")?;
    let constraint_reason: Option<String> = row.try_get("", "constraint_reason")?;
    let constraint_is_project_policy: Option<bool> =
        row.try_get("", "constraint_is_project_policy")?;
    let constraint_id: Option<Uuid> = row.try_get("", "constraint_id")?;
    let grant_id: Uuid = row.try_get("", "grant_id")?;

    let grant_id = PermissionGrantId::new(grant_id);
    let policy_constraint = parse_policy_constraint(
        constraint_reason,
        constraint_is_project_policy,
        constraint_id,
    )?;
    let effective_state = if policy_constraint.is_some() {
        PermissionEffectiveState::Constrained
    } else {
        PermissionEffectiveState::Granted
    };
    let permission = MyPermissionRecord {
        grant_id,
        permission: parse_permission_name(&permission_name)?,
        effective_state,
        grant_source: parse_grant_source(role_template_name.clone())?,
        policy_constraint,
    };
    let scope = parse_scope(org_id, project_id)?;
    let (scope_kind, scope_id) = parse_scope_key(org_id, project_id)?;

    Ok(ParsedPermissionRow {
        scope,
        permission,
        cursor: MyPermissionsCursor {
            org_id: if scope_kind == MyPermissionsScopeKind::Organization {
                Some(OrgId::new(scope_id))
            } else {
                None
            },
            project_id: if scope_kind == MyPermissionsScopeKind::Project {
                Some(ProjectId::new(scope_id))
            } else {
                None
            },
            permission_name,
            role_template_name,
            grant_id,
        },
    })
}

fn parse_scope(org_id: Option<Uuid>, project_id: Option<Uuid>) -> Result<Scope, StoreError> {
    match (org_id.map(OrgId::new), project_id.map(ProjectId::new)) {
        (Some(scope_org_id), None) => Ok(Scope::Organization(scope_org_id)),
        (None, Some(scope_project_id)) => Ok(Scope::Project(scope_project_id)),
        _ => Err(StoreError::Invariant {
            entity: "permission_grants",
            detail: "each grant must have exactly one scope (org or project)",
        }),
    }
}

fn parse_scope_key(
    org_id: Option<Uuid>,
    project_id: Option<Uuid>,
) -> Result<(MyPermissionsScopeKind, Uuid), StoreError> {
    match (org_id, project_id) {
        (Some(scope_org_id), None) => Ok((MyPermissionsScopeKind::Organization, scope_org_id)),
        (None, Some(scope_project_id)) => Ok((MyPermissionsScopeKind::Project, scope_project_id)),
        _ => Err(StoreError::Invariant {
            entity: "permission_grants",
            detail: "each grant must have exactly one scope (org or project)",
        }),
    }
}

#[derive(Debug, Clone, Copy)]
enum Scope {
    Organization(OrgId),
    Project(ProjectId),
}

#[derive(Debug, Clone)]
struct ParsedPermissionRow {
    scope: Scope,
    permission: MyPermissionRecord,
    cursor: MyPermissionsCursor,
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

fn introspection_query(
    backend: DbBackend,
    account_id: AccountId,
    cursor: Option<&MyPermissionsCursor>,
    limit: u16,
) -> Result<Statement, StoreError> {
    match cursor {
        None => Ok(introspection_query_first_page(backend, account_id, limit)),
        Some(cursor) => introspection_query_from_cursor(backend, account_id, cursor, limit),
    }
}

fn introspection_query_first_page(
    backend: DbBackend,
    account_id: AccountId,
    limit: u16,
) -> Statement {
    let sql = match backend {
        DbBackend::Postgres => INTROSPECTION_QUERY_FIRST_PAGE_POSTGRES,
        DbBackend::Sqlite | DbBackend::MySql => INTROSPECTION_QUERY_FIRST_PAGE_SQLITE,
    };
    Statement::from_sql_and_values(
        backend,
        sql,
        [account_id.as_uuid().into(), i64::from(limit).into()],
    )
}

fn introspection_query_from_cursor(
    backend: DbBackend,
    account_id: AccountId,
    cursor: &MyPermissionsCursor,
    limit: u16,
) -> Result<Statement, StoreError> {
    let sql = match backend {
        DbBackend::Postgres => INTROSPECTION_QUERY_FROM_CURSOR_POSTGRES,
        DbBackend::Sqlite | DbBackend::MySql => INTROSPECTION_QUERY_FROM_CURSOR_SQLITE,
    };
    let scope_kind = cursor.scope_kind()?;
    let scope_uuid = if let Some(org_id) = cursor.org_id {
        org_id.as_uuid()
    } else if let Some(project_id) = cursor.project_id {
        project_id.as_uuid()
    } else {
        return Err(StoreError::Invariant {
            entity: "permission_grants",
            detail: "cursor must carry exactly one scope id (org or project)",
        });
    };
    let cursor_role_template_name = cursor.role_template_name.clone().unwrap_or_default();
    let cursor_has_role_template = i32::from(cursor.role_template_name.is_some());
    Ok(Statement::from_sql_and_values(
        backend,
        sql,
        [
            account_id.as_uuid().into(),
            i32::from(scope_kind.rank()).into(),
            scope_uuid.into(),
            cursor.permission_name.clone().into(),
            cursor_has_role_template.into(),
            cursor_role_template_name.into(),
            cursor.grant_id.as_uuid().into(),
            i64::from(limit).into(),
        ],
    ))
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
    constraint_id: Option<Uuid>,
) -> Result<Option<PermissionConstraintRecord>, StoreError> {
    match (
        constraint_reason,
        constraint_is_project_policy,
        constraint_id,
    ) {
        (Some(reason), Some(is_project_policy), Some(id)) => Ok(Some(PermissionConstraintRecord {
            id: PermissionConstraintId::new(id),
            reason: parse_policy_constraint_reason(&reason)?,
            source: if is_project_policy {
                tanren_identity_policy::PolicyConstraintSource::ProjectPolicy
            } else {
                tanren_identity_policy::PolicyConstraintSource::OrganizationPolicy
            },
        })),
        (None, None, None) => Ok(None),
        _ => Err(StoreError::Invariant {
            entity: "permission_constraints",
            detail: "constraint id/reason/source columns must be all null or all set",
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

const INTROSPECTION_QUERY_FIRST_PAGE_POSTGRES: &str = r"
SELECT
  pg.org_id AS org_id,
  pg.project_id AS project_id,
  pg.permission_name,
  pg.role_template_name,
  pc.id AS constraint_id,
  pc.reason AS constraint_reason,
  pc.is_project_policy AS constraint_is_project_policy,
  pg.id AS grant_id
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

const INTROSPECTION_QUERY_FIRST_PAGE_SQLITE: &str = r"
SELECT
  pg.org_id AS org_id,
  pg.project_id AS project_id,
  pg.permission_name,
  pg.role_template_name,
  pc.id AS constraint_id,
  pc.reason AS constraint_reason,
  pc.is_project_policy AS constraint_is_project_policy,
  pg.id AS grant_id
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

const INTROSPECTION_QUERY_FROM_CURSOR_POSTGRES: &str = r"
SELECT
  pg.org_id AS org_id,
  pg.project_id AS project_id,
  pg.permission_name,
  pg.role_template_name,
  pc.id AS constraint_id,
  pc.reason AS constraint_reason,
  pc.is_project_policy AS constraint_is_project_policy,
  pg.id AS grant_id
FROM permission_grants pg
LEFT JOIN permission_constraints pc ON pc.grant_id = pg.id
WHERE pg.account_id = $1
  AND (
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
  ) > ($2, $3, $4, $5, $6, $7)
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
LIMIT $8
";

const INTROSPECTION_QUERY_FROM_CURSOR_SQLITE: &str = r"
SELECT
  pg.org_id AS org_id,
  pg.project_id AS project_id,
  pg.permission_name,
  pg.role_template_name,
  pc.id AS constraint_id,
  pc.reason AS constraint_reason,
  pc.is_project_policy AS constraint_is_project_policy,
  pg.id AS grant_id
FROM permission_grants pg
LEFT JOIN permission_constraints pc ON pc.grant_id = pg.id
WHERE pg.account_id = ?
  AND (
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
  ) > (?, ?, ?, ?, ?, ?)
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
