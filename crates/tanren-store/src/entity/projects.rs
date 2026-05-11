//! `SeaORM` entity for the `projects` table — minimal read-model fixture
//! for project listings scoped to an organization.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "projects")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub created_at: DateTimeUtc,
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
}

impl Related<super::organizations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
