//! `SeaORM` entity for the `deployment_postures` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "deployment_postures")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub scope_kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub scope_id: Uuid,
    pub posture: String,
    pub changed_by: Uuid,
    pub changed_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
