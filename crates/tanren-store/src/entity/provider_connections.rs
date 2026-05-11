//! `SeaORM` entity for the `provider_connections` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "provider_connections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub account_id: Uuid,
    pub provider_kind: String,
    pub provider_name: String,
    pub status: String,
    /// Opaque access credential — ciphertext, not a raw token value.
    /// The field name matches the xtask check-secrets pattern; a field
    /// exemption in secret-newtypes.toml documents why the column type
    /// is bare `String` (encrypted payload, not a plaintext secret).
    pub opaque_access_token: String,
    pub external_account_id: String,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
