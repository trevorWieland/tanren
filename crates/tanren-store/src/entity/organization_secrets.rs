//! `SeaORM` entity for the `organization_secrets` metadata table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "organization_secrets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub owner_scope: String,
    pub status: String,
    pub use_policy: String,
    pub version: i32,
    pub description: Option<String>,
    pub provider: Option<String>,
    pub created_by_account_id: Uuid,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    pub deleted_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::organizations::Entity",
        from = "Column::OrgId",
        to = "super::organizations::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Organization,
    #[sea_orm(
        belongs_to = "super::accounts::Entity",
        from = "Column::CreatedByAccountId",
        to = "super::accounts::Column::Id",
        on_update = "Cascade",
        on_delete = "Restrict"
    )]
    CreatedByAccount,
    #[sea_orm(has_many = "super::organization_secret_values::Entity")]
    Values,
}

impl Related<super::organizations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

impl Related<super::accounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CreatedByAccount.def()
    }
}

impl Related<super::organization_secret_values::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Values.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
