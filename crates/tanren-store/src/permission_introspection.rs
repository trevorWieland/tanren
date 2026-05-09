//! Read-model query for self-permission introspection.

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashMap;
use tanren_identity_policy::{
    AccountId, OrgId, PermissionEffectiveState, PermissionGrantSource, ProjectId,
};
use uuid::Uuid;

use crate::entity;
use crate::{
    MyOrganizationPermissionsRecord, MyPermissionRecord, MyPermissionsRecord,
    MyProjectPermissionsRecord, PermissionConstraintRecord, PermissionGrantRecord, StoreError,
};

pub(crate) async fn load_my_permissions(
    conn: &DatabaseConnection,
    account_id: AccountId,
) -> Result<MyPermissionsRecord, StoreError> {
    let grant_rows = entity::permission_grants::Entity::find()
        .filter(entity::permission_grants::Column::AccountId.eq(account_id.as_uuid()))
        .all(conn)
        .await?;
    let mut grants: Vec<PermissionGrantRecord> = grant_rows
        .into_iter()
        .map(PermissionGrantRecord::from)
        .collect();

    if grants.is_empty() {
        return Ok(MyPermissionsRecord {
            organizations: Vec::new(),
            projects: Vec::new(),
        });
    }

    let grant_ids: Vec<Uuid> = grants.iter().map(|grant| grant.id.as_uuid()).collect();
    let constraint_rows = entity::permission_constraints::Entity::find()
        .filter(entity::permission_constraints::Column::GrantId.is_in(grant_ids))
        .all(conn)
        .await?;
    let mut constraints_by_grant = constraints_by_grant(constraint_rows)?;

    // Sort upfront so grouped output stays deterministic across backends.
    grants.sort_by(compare_permission_grants);

    let mut organization_permissions: HashMap<Uuid, Vec<MyPermissionRecord>> =
        HashMap::with_capacity(grants.len());
    let mut project_permissions: HashMap<Uuid, Vec<MyPermissionRecord>> =
        HashMap::with_capacity(grants.len());

    for grant in grants {
        let policy_constraint = constraints_by_grant.remove(&grant.id.as_uuid());
        let effective_state = if policy_constraint.is_some() {
            PermissionEffectiveState::Constrained
        } else {
            PermissionEffectiveState::Granted
        };
        let permission = MyPermissionRecord {
            permission: grant.permission,
            effective_state,
            grant_source: grant.grant_source,
            policy_constraint,
        };

        match (grant.org_id, grant.project_id) {
            (Some(org_id), None) => organization_permissions
                .entry(org_id.as_uuid())
                .or_default()
                .push(permission),
            (None, Some(project_id)) => project_permissions
                .entry(project_id.as_uuid())
                .or_default()
                .push(permission),
            _ => {
                return Err(StoreError::Invariant {
                    entity: "permission_grants",
                    detail: "each grant must have exactly one scope (org or project)",
                });
            }
        }
    }

    let organizations = project_sorted_organizations(organization_permissions)?;
    let projects = project_sorted_projects(project_permissions)?;
    Ok(MyPermissionsRecord {
        organizations,
        projects,
    })
}

fn compare_permission_grants(
    left: &PermissionGrantRecord,
    right: &PermissionGrantRecord,
) -> std::cmp::Ordering {
    let left_scope = match (left.org_id, left.project_id) {
        (Some(org_id), None) => (0_u8, org_id.as_uuid()),
        (None, Some(project_id)) => (1_u8, project_id.as_uuid()),
        _ => (2_u8, left.id.as_uuid()),
    };
    let right_scope = match (right.org_id, right.project_id) {
        (Some(org_id), None) => (0_u8, org_id.as_uuid()),
        (None, Some(project_id)) => (1_u8, project_id.as_uuid()),
        _ => (2_u8, right.id.as_uuid()),
    };
    let left_source = permission_grant_source_sort_key(&left.grant_source);
    let right_source = permission_grant_source_sort_key(&right.grant_source);

    left_scope
        .cmp(&right_scope)
        .then_with(|| left.permission.as_str().cmp(right.permission.as_str()))
        .then_with(|| left_source.cmp(&right_source))
        .then_with(|| left.id.cmp(&right.id))
}

fn permission_grant_source_sort_key(source: &PermissionGrantSource) -> (u8, &str) {
    match source {
        PermissionGrantSource::Direct => (0, ""),
        PermissionGrantSource::RoleTemplate { role_template } => (1, role_template.as_str()),
    }
}

fn project_sorted_organizations(
    mut by_org: HashMap<Uuid, Vec<MyPermissionRecord>>,
) -> Result<Vec<MyOrganizationPermissionsRecord>, StoreError> {
    let mut org_ids: Vec<Uuid> = by_org.keys().copied().collect();
    org_ids.sort_unstable();
    let mut organizations = Vec::with_capacity(org_ids.len());
    for org_id in org_ids {
        let Some(permissions) = by_org.remove(&org_id) else {
            return Err(StoreError::Invariant {
                entity: "permission_grants",
                detail: "organization permission bucket missing during projection",
            });
        };
        organizations.push(MyOrganizationPermissionsRecord {
            org_id: OrgId::new(org_id),
            permissions,
        });
    }
    Ok(organizations)
}

fn project_sorted_projects(
    mut by_project: HashMap<Uuid, Vec<MyPermissionRecord>>,
) -> Result<Vec<MyProjectPermissionsRecord>, StoreError> {
    let mut project_ids: Vec<Uuid> = by_project.keys().copied().collect();
    project_ids.sort_unstable();
    let mut projects = Vec::with_capacity(project_ids.len());
    for project_id in project_ids {
        let Some(permissions) = by_project.remove(&project_id) else {
            return Err(StoreError::Invariant {
                entity: "permission_grants",
                detail: "project permission bucket missing during projection",
            });
        };
        projects.push(MyProjectPermissionsRecord {
            project_id: ProjectId::new(project_id),
            permissions,
        });
    }
    Ok(projects)
}

fn constraints_by_grant(
    rows: Vec<entity::permission_constraints::Model>,
) -> Result<HashMap<Uuid, PermissionConstraintRecord>, StoreError> {
    let mut constraints_by_grant = HashMap::with_capacity(rows.len());
    for row in rows {
        let grant_id = row.grant_id;
        let previous = constraints_by_grant.insert(grant_id, PermissionConstraintRecord::from(row));
        if previous.is_some() {
            return Err(StoreError::Invariant {
                entity: "permission_constraints",
                detail: "at most one constraint row is allowed per grant",
            });
        }
    }
    Ok(constraints_by_grant)
}
