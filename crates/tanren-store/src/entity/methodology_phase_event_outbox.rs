use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "methodology_phase_event_outbox")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub event_id: Uuid,
    pub spec_id: Uuid,
    pub spec_folder: String,
    pub line_json: String,
    pub status: String,
    pub attempt_count: i32,
    pub last_error: Option<String>,
    pub created_at: DateTimeUtc,
    pub projected_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
