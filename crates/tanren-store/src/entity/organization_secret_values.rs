//! `SeaORM` entity for the `organization_secret_values` encrypted-value table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "organization_secret_values")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub secret_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub version: i32,
    pub encrypted_value: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_id: String,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::organization_secrets::Entity",
        from = "Column::SecretId",
        to = "super::organization_secrets::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Secret,
}

impl Related<super::organization_secrets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Secret.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
