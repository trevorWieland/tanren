use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tanren_identity_policy::{AccountId, LoopId, MilestoneId, ProjectId, SpecId};

use crate::entity;
use crate::{
    ActiveProjectRecord, ProjectRecord, ProjectScopedViewRecord, ProjectStore, SpecRecord,
    StoreError,
};

#[async_trait]
impl ProjectStore for crate::Store {
    async fn list_projects(&self, account_id: AccountId) -> Result<Vec<ProjectRecord>, StoreError> {
        let rows = entity::projects::Entity::find()
            .filter(entity::projects::Column::AccountId.eq(account_id.as_uuid()))
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(ProjectRecord::from).collect())
    }

    async fn find_attention_specs(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<SpecRecord>, StoreError> {
        let rows = entity::specs::Entity::find()
            .filter(entity::specs::Column::ProjectId.eq(project_id.as_uuid()))
            .filter(entity::specs::Column::NeedsAttention.eq(true))
            .all(&self.conn)
            .await?;
        Ok(rows.into_iter().map(SpecRecord::from).collect())
    }

    async fn read_active_project(
        &self,
        account_id: AccountId,
    ) -> Result<Option<ActiveProjectRecord>, StoreError> {
        let row = entity::active_projects::Entity::find_by_id(account_id.as_uuid())
            .one(&self.conn)
            .await?;
        Ok(row.map(ActiveProjectRecord::from))
    }

    async fn write_active_project(
        &self,
        account_id: AccountId,
        project_id: ProjectId,
        now: DateTime<Utc>,
    ) -> Result<ActiveProjectRecord, StoreError> {
        let project = entity::projects::Entity::find_by_id(project_id.as_uuid())
            .one(&self.conn)
            .await?;
        match project {
            None => {
                return Err(StoreError::UnauthorizedProjectAccess);
            }
            Some(p) => {
                if p.account_id != account_id.as_uuid() {
                    return Err(StoreError::UnauthorizedProjectAccess);
                }
            }
        }

        let model = entity::active_projects::ActiveModel {
            account_id: Set(account_id.as_uuid()),
            project_id: Set(project_id.as_uuid()),
            switched_at: Set(now),
        };
        model.save(&self.conn).await?;
        Ok(ActiveProjectRecord {
            account_id,
            project_id,
            switched_at: now,
        })
    }

    async fn read_scoped_views(
        &self,
        project_id: ProjectId,
    ) -> Result<ProjectScopedViewRecord, StoreError> {
        let spec_rows = entity::specs::Entity::find()
            .filter(entity::specs::Column::ProjectId.eq(project_id.as_uuid()))
            .all(&self.conn)
            .await?;
        let spec_ids = spec_rows.into_iter().map(|r| SpecId::new(r.id)).collect();

        let loop_rows = entity::loops::Entity::find()
            .filter(entity::loops::Column::ProjectId.eq(project_id.as_uuid()))
            .all(&self.conn)
            .await?;
        let loop_ids = loop_rows.into_iter().map(|r| LoopId::new(r.id)).collect();

        let milestone_rows = entity::milestones::Entity::find()
            .filter(entity::milestones::Column::ProjectId.eq(project_id.as_uuid()))
            .all(&self.conn)
            .await?;
        let milestone_ids = milestone_rows
            .into_iter()
            .map(|r| MilestoneId::new(r.id))
            .collect();

        Ok(ProjectScopedViewRecord {
            project_id,
            spec_ids,
            loop_ids,
            milestone_ids,
        })
    }

    async fn read_view_state(
        &self,
        account_id: AccountId,
        project_id: ProjectId,
    ) -> Result<Option<serde_json::Value>, StoreError> {
        let row = find_view_state_row(&self.conn, account_id, project_id).await?;
        Ok(row.map(|r| r.view_state))
    }

    async fn write_view_state(
        &self,
        account_id: AccountId,
        project_id: ProjectId,
        view_state: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let existing = find_view_state_row(&self.conn, account_id, project_id).await?;
        if let Some(model) = existing {
            let mut active: entity::project_view_states::ActiveModel = model.into();
            active.view_state = Set(view_state);
            active.updated_at = Set(now);
            active.update(&self.conn).await?;
        } else {
            use uuid::Uuid as UuidExt;
            let model = entity::project_view_states::ActiveModel {
                id: Set(UuidExt::now_v7()),
                account_id: Set(account_id.as_uuid()),
                project_id: Set(project_id.as_uuid()),
                view_state: Set(view_state),
                updated_at: Set(now),
            };
            model.insert(&self.conn).await?;
        }
        Ok(())
    }
}

async fn find_view_state_row(
    conn: &DatabaseConnection,
    account_id: AccountId,
    project_id: ProjectId,
) -> Result<Option<entity::project_view_states::Model>, StoreError> {
    let row = entity::project_view_states::Entity::find()
        .filter(entity::project_view_states::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::project_view_states::Column::ProjectId.eq(project_id.as_uuid()))
        .one(conn)
        .await?;
    Ok(row)
}
