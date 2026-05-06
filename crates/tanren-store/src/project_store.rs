//! `SeaORM`-backed adapter for the [`ProjectStore`] port.
//!
//! Lives in its own module so `lib.rs` stays under the workspace per-file
//! line budget.

use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, ProviderConnectionId};
use uuid::Uuid;

use crate::Store;
use crate::entity;
use crate::traits::{
    ActiveLoopRead, ConnectProjectAtomicOutput, ConnectProjectAtomicRequest, DependencyLinkStatus,
    DependencyProjection, DisconnectProjectAtomicOutput, DisconnectProjectAtomicRequest,
    DisconnectProjectError, ProjectDependencyLink, ProjectStore, ReconnectProjectAtomicOutput,
    ReconnectProjectAtomicRequest, ReconnectProjectError, ReconnectedProject, SpecProjection,
};
use crate::{ProjectDependencyRecord, ProjectRecord, ProjectSpecRecord, StoreError};

#[async_trait]
impl ActiveLoopRead for Store {
    async fn has_active_loop(&self, project_id: ProjectId) -> Result<bool, StoreError> {
        #[cfg(feature = "test-hooks")]
        {
            let count = entity::project_loop_fixtures::Entity::find()
                .filter(entity::project_loop_fixtures::Column::ProjectId.eq(project_id.as_uuid()))
                .filter(entity::project_loop_fixtures::Column::IsActive.eq(true))
                .count(&self.conn)
                .await?;
            Ok(count > 0)
        }
        #[cfg(not(feature = "test-hooks"))]
        {
            let _ = project_id;
            Ok(false)
        }
    }
}

#[async_trait]
impl ProjectStore for Store {
    async fn connect_project_atomic(
        &self,
        request: ConnectProjectAtomicRequest,
    ) -> Result<ConnectProjectAtomicOutput, StoreError> {
        crate::project_lifecycle::connect_atomic(&self.conn, request).await
    }

    async fn find_project_by_org_and_resource(
        &self,
        org_id: OrgId,
        provider_connection_id: ProviderConnectionId,
        resource_id: &str,
    ) -> Result<Option<ProjectRecord>, StoreError> {
        let row = entity::projects::Entity::find()
            .filter(entity::projects::Column::OrgId.eq(org_id.as_uuid()))
            .filter(
                entity::projects::Column::ProviderConnectionId.eq(provider_connection_id.as_uuid()),
            )
            .filter(entity::projects::Column::ResourceId.eq(resource_id))
            .one(&self.conn)
            .await?;
        Ok(row.map(ProjectRecord::from))
    }

    async fn reconnect_project(
        &self,
        project_id: ProjectId,
    ) -> Result<ReconnectedProject, ReconnectProjectError> {
        let row = entity::projects::Entity::find_by_id(project_id.as_uuid())
            .one(&self.conn)
            .await
            .map_err(StoreError::from)?
            .ok_or(ReconnectProjectError::NotFound)?;

        let mut active: entity::projects::ActiveModel = row.into();
        active.disconnected_at = Set(None);
        let updated = active.update(&self.conn).await.map_err(StoreError::from)?;

        Ok(ReconnectedProject {
            project: ProjectRecord::from(updated),
        })
    }

    async fn reconnect_project_atomic(
        &self,
        request: ReconnectProjectAtomicRequest,
    ) -> Result<ReconnectProjectAtomicOutput, ReconnectProjectError> {
        crate::project_lifecycle::reconnect_atomic(&self.conn, request).await
    }

    async fn list_connected_projects_for_account(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<ProjectRecord>, StoreError> {
        let org_ids: Vec<Uuid> = entity::memberships::Entity::find()
            .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|m| m.org_id)
            .collect();

        if org_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = entity::projects::Entity::find()
            .filter(entity::projects::Column::OrgId.is_in(org_ids))
            .filter(entity::projects::Column::DisconnectedAt.is_null())
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(ProjectRecord::from).collect())
    }

    async fn disconnect_project_atomic(
        &self,
        request: DisconnectProjectAtomicRequest,
    ) -> Result<DisconnectProjectAtomicOutput, DisconnectProjectError> {
        crate::project_lifecycle::disconnect_atomic(&self.conn, request).await
    }

    async fn account_can_see_project(
        &self,
        account_id: AccountId,
        project_id: ProjectId,
    ) -> Result<bool, StoreError> {
        let org_ids: Vec<Uuid> = entity::memberships::Entity::find()
            .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|m| m.org_id)
            .collect();

        if org_ids.is_empty() {
            return Ok(false);
        }

        let count = entity::projects::Entity::find()
            .filter(entity::projects::Column::Id.eq(project_id.as_uuid()))
            .filter(entity::projects::Column::OrgId.is_in(org_ids))
            .count(&self.conn)
            .await?;
        Ok(count > 0)
    }

    async fn account_org_memberships(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<OrgId>, StoreError> {
        let org_ids: Vec<OrgId> = entity::memberships::Entity::find()
            .filter(entity::memberships::Column::AccountId.eq(account_id.as_uuid()))
            .all(&self.conn)
            .await?
            .into_iter()
            .map(|m| OrgId::new(m.org_id))
            .collect();
        Ok(org_ids)
    }
}

#[async_trait]
impl SpecProjection for Store {
    async fn read_project_specs(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectSpecRecord>, StoreError> {
        read_specs(&self.conn, project_id).await
    }
}

#[async_trait]
impl DependencyProjection for Store {
    async fn read_project_dependencies(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectDependencyLink>, StoreError> {
        let deps = entity::project_dependencies::Entity::find()
            .filter(entity::project_dependencies::Column::SourceProjectId.eq(project_id.as_uuid()))
            .all(&self.conn)
            .await?;

        resolve_dep_statuses(&self.conn, deps).await
    }

    async fn read_inbound_dependencies(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectDependencyLink>, StoreError> {
        let deps = entity::project_dependencies::Entity::find()
            .filter(entity::project_dependencies::Column::TargetProjectId.eq(project_id.as_uuid()))
            .all(&self.conn)
            .await?;

        resolve_dep_statuses(&self.conn, deps).await
    }
}

async fn read_specs(
    conn: &DatabaseConnection,
    project_id: ProjectId,
) -> Result<Vec<ProjectSpecRecord>, StoreError> {
    let rows = entity::project_specs::Entity::find()
        .filter(entity::project_specs::Column::ProjectId.eq(project_id.as_uuid()))
        .all(conn)
        .await?;
    Ok(rows.into_iter().map(ProjectSpecRecord::from).collect())
}

async fn resolve_dep_statuses(
    conn: &DatabaseConnection,
    deps: Vec<entity::project_dependencies::Model>,
) -> Result<Vec<ProjectDependencyLink>, StoreError> {
    let mut links = Vec::with_capacity(deps.len());
    for dep in deps {
        let target = entity::projects::Entity::find_by_id(dep.target_project_id)
            .one(conn)
            .await?;

        let status = match target {
            Some(p) if p.disconnected_at.is_none() => DependencyLinkStatus::Resolved,
            Some(_) => DependencyLinkStatus::TargetDisconnected,
            None => DependencyLinkStatus::TargetUnknown,
        };

        links.push(ProjectDependencyLink {
            dependency: ProjectDependencyRecord::from(dep),
            status,
        });
    }

    Ok(links)
}
