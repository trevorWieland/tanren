//! `SeaORM` entity for the `project_command_reservations` table.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "project_command_reservations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub owning_account_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub provider_family: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub repository_ref: String,
    pub status: String,
    pub active_reservation_id: Option<Uuid>,
    pub reserved_at: Option<DateTimeUtc>,
    pub lease_expires_at: Option<DateTimeUtc>,
    pub failure_count: i32,
    pub blocked_until: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
