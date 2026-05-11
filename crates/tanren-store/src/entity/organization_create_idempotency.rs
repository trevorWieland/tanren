//! `SeaORM` entity for organization-create idempotency records.

use sea_orm::entity::prelude::*;

#[derive(Clone, PartialEq, Eq, DeriveEntityModel)]
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

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("organization_create_idempotency::Model")
            .field("account_id", &self.account_id)
            .field("key", &"<redacted>")
            .field("organization_id", &self.organization_id)
            .field("organization_name", &self.organization_name)
            .field("request_fingerprint", &"<redacted>")
            .field("created_at", &self.created_at)
            .finish()
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::accounts::Entity",
        from = "Column::AccountId",
        to = "super::accounts::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Account,
    #[sea_orm(
        belongs_to = "super::organizations::Entity",
        from = "Column::OrganizationId",
        to = "super::organizations::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Organization,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl Related<super::organizations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
