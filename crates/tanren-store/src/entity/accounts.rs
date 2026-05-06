//! `SeaORM` entity for the `accounts` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "accounts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(unique)]
    pub identifier: String,
    pub display_name: String,
    /// PHC-format hash string written by `Argon2idVerifier::hash`
    /// (e.g. `$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`). The salt
    /// is embedded in the string — there is no separate salt column.
    pub password_phc: String,
    pub created_at: DateTimeUtc,
    pub org_id: Option<Uuid>,
    pub active_org_id: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
