//! `SeaORM` entity for organization-create idempotency records.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "organization_create_idempotency")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub account_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false, column_name = "idempotency_key")]
    pub key: String,
    pub organization_id: Uuid,
    pub organization_name: String,
    pub request_fingerprint: String,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
