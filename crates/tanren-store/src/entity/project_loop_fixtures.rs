//! `SeaORM` entity for the `project_loop_fixtures` table.
//!
//! M-0003 fixture seam for the no-active-loops precondition. M-0011
//! owns real implementation loops; this table is replaced when that
//! module lands.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "project_loop_fixtures")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub project_id: Uuid,
    pub is_active: bool,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
