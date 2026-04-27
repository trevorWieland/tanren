use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "actor_token_replay")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub issuer: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub audience: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub jti: String,
    pub iat_unix: i64,
    pub exp_unix: i64,
    pub consumed_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
