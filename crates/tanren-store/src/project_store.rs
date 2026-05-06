//! `SeaORM`-backed adapter for the [`ProjectStore`] port.
//!
//! Lives in its own module so `lib.rs` stays under the workspace per-file
//! line budget.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    Set,
};
use tanren_identity_policy::{AccountId, OrgId, ProjectId};
use uuid::Uuid;

use crate::Store;
use crate::entity;
use crate::traits::{
    DependencyLinkStatus, DisconnectProjectError, ProjectDependencyLink, ProjectStore,
    ReconnectProjectError, ReconnectedProject,
};
use crate::{
    ProjectDependencyRecord, ProjectLoopFixtureRecord, ProjectRecord, ProjectSpecRecord, StoreError,
};

#[async_trait]
impl ProjectStore for Store {
    async fn insert_project(
        &self,
        project_id: ProjectId,
        org_id: OrgId,
        name: String,
        repository_url: String,
        now: DateTime<Utc>,
    ) -> Result<ProjectRecord, StoreError> {
        let model = entity::projects::ActiveModel {
            id: Set(project_id.as_uuid()),
            org_id: Set(org_id.as_uuid()),
            name: Set(name),
            repository_url: Set(repository_url),
            connected_at: Set(now),
            disconnected_at: Set(None),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(ProjectRecord::from(inserted))
    }

    async fn find_project_by_org_and_repo(
        &self,
        org_id: OrgId,
        repository_url: &str,
    ) -> Result<Option<ProjectRecord>, StoreError> {
        let row = entity::projects::Entity::find()
            .filter(entity::projects::Column::OrgId.eq(org_id.as_uuid()))
            .filter(entity::projects::Column::RepositoryUrl.eq(repository_url))
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

        let specs = read_specs(&self.conn, project_id).await?;

        Ok(ReconnectedProject {
            project: ProjectRecord::from(updated),
            specs,
        })
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

    async fn disconnect_project(
        &self,
        project_id: ProjectId,
        now: DateTime<Utc>,
    ) -> Result<ProjectRecord, DisconnectProjectError> {
        let row = entity::projects::Entity::find_by_id(project_id.as_uuid())
            .one(&self.conn)
            .await
            .map_err(StoreError::from)?
            .ok_or(DisconnectProjectError::NotFound)?;

        let mut active: entity::projects::ActiveModel = row.into();
        active.disconnected_at = Set(Some(now));
        let updated = active.update(&self.conn).await.map_err(StoreError::from)?;

        Ok(ProjectRecord::from(updated))
    }

    async fn read_project_specs(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<ProjectSpecRecord>, StoreError> {
        read_specs(&self.conn, project_id).await
    }

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

    async fn set_loop_fixture(
        &self,
        project_id: ProjectId,
        is_active: bool,
        now: DateTime<Utc>,
    ) -> Result<ProjectLoopFixtureRecord, StoreError> {
        let id = Uuid::now_v7();
        let model = entity::project_loop_fixtures::ActiveModel {
            id: Set(id),
            project_id: Set(project_id.as_uuid()),
            is_active: Set(is_active),
            created_at: Set(now),
        };
        model.insert(&self.conn).await?;
        Ok(ProjectLoopFixtureRecord {
            id,
            project_id,
            is_active,
            created_at: now,
        })
    }

    async fn has_active_loop_fixtures(&self, project_id: ProjectId) -> Result<bool, StoreError> {
        let count = entity::project_loop_fixtures::Entity::find()
            .filter(entity::project_loop_fixtures::Column::ProjectId.eq(project_id.as_uuid()))
            .filter(entity::project_loop_fixtures::Column::IsActive.eq(true))
            .count(&self.conn)
            .await?;
        Ok(count > 0)
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
