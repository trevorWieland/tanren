use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "organization_create_idempotency")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub request_id: String,
    pub account_id: Uuid,
    pub canonical_name: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub response_json: Json,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
