use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "methodology_idempotency")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tool: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub scope_key: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub idempotency_key: String,
    pub request_hash: String,
    pub request_hash_algo: String,
    pub reservation_expires_at: Option<DateTimeUtc>,
    pub response_json: Option<String>,
    pub first_event_id: Option<Uuid>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
