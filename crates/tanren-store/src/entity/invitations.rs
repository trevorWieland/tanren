//! `SeaORM` entity for the `invitations` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "invitations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub token: String,
    pub inviting_org_id: Uuid,
    pub expires_at: DateTimeUtc,
    pub consumed_at: Option<DateTimeUtc>,
    pub target_identifier: Option<String>,
    pub org_permissions: Option<String>,
    pub revoked_at: Option<DateTimeUtc>,
    pub revoked_by: Option<Uuid>,
    pub consumed_by: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
