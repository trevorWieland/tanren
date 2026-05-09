//! `SeaORM` entity for direct permission grants.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "permission_grants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub grantee_kind: String,
    pub grantee_ref: Uuid,
    pub scope_kind: String,
    pub scope_ref: Uuid,
    pub permission_name: String,
    pub source_role_id: Uuid,
    pub granted_by_kind: String,
    pub granted_by_ref: Uuid,
    pub granted_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
