//! `SeaORM` entity for the `projects` table — registered Tanren projects.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub repository_id: Uuid,
    pub owner_account_id: Uuid,
    pub owner_org_id: Option<Uuid>,
    pub repository_identity: String,
    pub repository_url: String,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
