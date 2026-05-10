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
    let source_checkpoint_before_read = load_source_checkpoint(conn, account_id).await?;
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
    let source_checkpoint_after_read = load_source_checkpoint(conn, account_id).await?;
    let staleness = derive_staleness(
        &source_checkpoint_before_read,
        &source_checkpoint_after_read,
    );

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
        read_metadata: crate::MyPermissionsReadMetaRecord {
            source: "permission_introspection_permission_grants_table_v1".to_owned(),
            generated_at,
            source_checkpoint: source_checkpoint_after_read,
            staleness,
        },
        organizations,
        projects,
    })
}

fn derive_staleness(
    source_checkpoint_before_read: &crate::MyPermissionsSourceCheckpointRecord,
    source_checkpoint_after_read: &crate::MyPermissionsSourceCheckpointRecord,
) -> crate::MyPermissionsStalenessRecord {
    if source_checkpoint_before_read == source_checkpoint_after_read {
        crate::MyPermissionsStalenessRecord::Fresh
    } else {
        crate::MyPermissionsStalenessRecord::PotentiallyStale
    }
}

async fn load_source_checkpoint(
    conn: &DatabaseConnection,
    account_id: AccountId,
) -> Result<crate::MyPermissionsSourceCheckpointRecord, StoreError> {
    let query = source_checkpoint_query(conn.get_database_backend(), account_id);
    let row = conn.query_one(query).await?.ok_or(StoreError::Invariant {
        entity: "permission_grants",
        detail: "source checkpoint query returned no row",
    })?;
    let max_permission_grant_id: Option<Uuid> = row.try_get("", "max_permission_grant_id")?;
    let max_permission_constraint_id: Option<Uuid> =
        row.try_get("", "max_permission_constraint_id")?;

    Ok(crate::MyPermissionsSourceCheckpointRecord {
        max_permission_grant_id: max_permission_grant_id.map(PermissionGrantId::new),
        max_permission_constraint_id: max_permission_constraint_id.map(PermissionConstraintId::new),
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
    Statement::from_sql_and_values(
        backend,
        introspection_query_first_page_sql(backend),
        [account_id.as_uuid().into(), i64::from(limit).into()],
    )
}

fn introspection_query_from_cursor(
    backend: DbBackend,
    account_id: AccountId,
    cursor: &MyPermissionsCursor,
    limit: u16,
) -> Result<Statement, StoreError> {
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
        introspection_query_from_cursor_sql(backend),
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

fn introspection_query_first_page_sql(backend: DbBackend) -> String {
    let query_dialect = introspection_query_dialect(backend);
    let base_sql = introspection_query_base_sql(query_dialect.account);
    let first_page_limit = query_dialect.first_page_limit;
    let order_by = INTROSPECTION_QUERY_ORDER_BY;

    format!(
        r"{base_sql}
{order_by}
LIMIT {first_page_limit}
"
    )
}

fn introspection_query_from_cursor_sql(backend: DbBackend) -> String {
    let query_dialect = introspection_query_dialect(backend);
    let base_sql = introspection_query_base_sql(query_dialect.account);
    let cursor_tuple = query_dialect.cursor_tuple;
    let cursor_page_limit = query_dialect.cursor_page_limit;
    let order_cursor_tuple = INTROSPECTION_QUERY_ORDER_CURSOR_TUPLE;
    let order_by = INTROSPECTION_QUERY_ORDER_BY;

    format!(
        r"{base_sql}
  AND {order_cursor_tuple} > {cursor_tuple}
{order_by}
LIMIT {cursor_page_limit}
"
    )
}

fn introspection_query_base_sql(account_placeholder: &str) -> String {
    format!(
        r"{INTROSPECTION_QUERY_SELECT_JOIN}
WHERE pg.account_id = {account_placeholder}",
    )
}

fn source_checkpoint_query(backend: DbBackend, account_id: AccountId) -> Statement {
    Statement::from_sql_and_values(
        backend,
        source_checkpoint_query_sql(backend),
        [account_id.as_uuid().into()],
    )
}

fn source_checkpoint_query_sql(backend: DbBackend) -> String {
    let account_placeholder = match backend {
        DbBackend::Postgres => "$1",
        DbBackend::Sqlite | DbBackend::MySql => "?",
    };
    format!(
        r"
SELECT
  MAX(pg.id) AS max_permission_grant_id,
  MAX(pc.id) AS max_permission_constraint_id
FROM permission_grants pg
LEFT JOIN permission_constraints pc ON pc.grant_id = pg.id
WHERE pg.account_id = {account_placeholder}
"
    )
}

fn introspection_query_dialect(backend: DbBackend) -> IntrospectionQueryDialect {
    match backend {
        DbBackend::Postgres => IntrospectionQueryDialect {
            account: "$1",
            first_page_limit: "$2",
            cursor_tuple: "($2, $3, $4, $5, $6, $7)",
            cursor_page_limit: "$8",
        },
        DbBackend::Sqlite | DbBackend::MySql => IntrospectionQueryDialect {
            account: "?",
            first_page_limit: "?",
            cursor_tuple: "(?, ?, ?, ?, ?, ?)",
            cursor_page_limit: "?",
        },
    }
}

struct IntrospectionQueryDialect {
    account: &'static str,
    first_page_limit: &'static str,
    cursor_tuple: &'static str,
    cursor_page_limit: &'static str,
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

const INTROSPECTION_QUERY_SELECT_JOIN: &str = r"
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
";

const INTROSPECTION_QUERY_ORDER_CURSOR_TUPLE: &str = r"
(
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
)";

const INTROSPECTION_QUERY_ORDER_BY: &str = r"
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
";
