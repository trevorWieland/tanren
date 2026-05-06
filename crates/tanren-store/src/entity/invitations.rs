//! `SeaORM` entity for the `invitations` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "invitations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub token: String,
    pub inviting_org_id: Uuid,
    pub recipient_identifier: String,
    #[sea_orm(column_type = "JsonBinary")]
    pub granted_permissions: serde_json::Value,
    pub created_by_account_id: Uuid,
    pub created_at: DateTimeUtc,
    pub expires_at: DateTimeUtc,
    pub consumed_at: Option<DateTimeUtc>,
    pub revoked_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
