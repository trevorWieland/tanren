use sea_orm::entity::prelude::*;

/// Current task-status projection materialized from methodology events.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "methodology_task_status")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: Uuid,
    pub spec_id: Uuid,
    pub status: String,
    pub gate_checked: bool,
    pub audited: bool,
    pub adherent: bool,
    #[sea_orm(column_type = "JsonBinary")]
    pub extra_guards: Json,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
