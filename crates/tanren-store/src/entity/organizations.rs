//! `SeaORM` entity for the `organizations` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "organizations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    /// Normalized global organization-name uniqueness key.
    pub name: String,
    pub created_by_account_id: Uuid,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::accounts::Entity",
        from = "Column::CreatedByAccountId",
        to = "super::accounts::Column::Id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    CreatedByAccount,
    #[sea_orm(has_many = "super::organization_permission_grants::Entity")]
    PermissionGrants,
    #[sea_orm(has_many = "super::organization_create_idempotency::Entity")]
    CreateIdempotencyClaims,
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CreatedByAccount.def()
    }
}

impl Related<super::organization_permission_grants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PermissionGrants.def()
    }
}

impl Related<super::organization_create_idempotency::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CreateIdempotencyClaims.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
