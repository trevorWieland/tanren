use sea_orm::{ActiveModelTrait, Set};

use crate::entity;
use crate::{
    LoopRecord, MilestoneRecord, NewLoop, NewMilestone, NewProject, NewSpec, ProjectRecord,
    SpecRecord, StoreError,
};

impl crate::Store {
    pub async fn seed_project(&self, new: NewProject) -> Result<ProjectRecord, StoreError> {
        let model = entity::projects::ActiveModel {
            id: Set(new.id.as_uuid()),
            account_id: Set(new.account_id.as_uuid()),
            name: Set(new.name),
            state: Set(new.state),
            created_at: Set(new.created_at),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(ProjectRecord::from(inserted))
    }

    pub async fn seed_spec(&self, new: NewSpec) -> Result<SpecRecord, StoreError> {
        let model = entity::specs::ActiveModel {
            id: Set(new.id.as_uuid()),
            project_id: Set(new.project_id.as_uuid()),
            name: Set(new.name),
            needs_attention: Set(new.needs_attention),
            attention_reason: Set(new.attention_reason),
            created_at: Set(new.created_at),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(SpecRecord::from(inserted))
    }

    pub async fn seed_loop(&self, new: NewLoop) -> Result<LoopRecord, StoreError> {
        let model = entity::loops::ActiveModel {
            id: Set(new.id.as_uuid()),
            project_id: Set(new.project_id.as_uuid()),
            created_at: Set(new.created_at),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(LoopRecord::from(inserted))
    }

    pub async fn seed_milestone(&self, new: NewMilestone) -> Result<MilestoneRecord, StoreError> {
        let model = entity::milestones::ActiveModel {
            id: Set(new.id.as_uuid()),
            project_id: Set(new.project_id.as_uuid()),
            created_at: Set(new.created_at),
        };
        let inserted = model.insert(&self.conn).await?;
        Ok(MilestoneRecord::from(inserted))
    }
}
