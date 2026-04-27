use sea_orm::entity::prelude::*;

/// Projection row mapping (task, finding) pairs under one spec.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "methodology_task_finding")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub finding_id: Uuid,
    pub spec_id: Uuid,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
