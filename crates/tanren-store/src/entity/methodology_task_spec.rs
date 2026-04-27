use sea_orm::entity::prelude::*;

/// Projection row mapping a task root to its owning spec.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "methodology_task_spec")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: Uuid,
    pub spec_id: Uuid,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
