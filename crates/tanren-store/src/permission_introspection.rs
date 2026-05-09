//! Read-model query for self-permission introspection.

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::{BTreeMap, HashMap};
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
    let mut constraints_by_grant: HashMap<Uuid, PermissionConstraintRecord> = constraint_rows
        .into_iter()
        .map(|constraint| {
            (
                constraint.grant_id,
                PermissionConstraintRecord::from(constraint),
            )
        })
        .collect();

    // Sort upfront so grouped output stays deterministic across backends.
    grants.sort_by(compare_permission_grants);

    let mut organization_permissions: BTreeMap<Uuid, Vec<MyPermissionRecord>> = BTreeMap::new();
    let mut project_permissions: BTreeMap<Uuid, Vec<MyPermissionRecord>> = BTreeMap::new();

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

    let organizations = organization_permissions
        .into_iter()
        .map(|(org_id, permissions)| MyOrganizationPermissionsRecord {
            org_id: OrgId::new(org_id),
            permissions,
        })
        .collect();
    let projects = project_permissions
        .into_iter()
        .map(|(project_id, permissions)| MyProjectPermissionsRecord {
            project_id: ProjectId::new(project_id),
            permissions,
        })
        .collect();
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
