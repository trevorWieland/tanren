use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0009_actor_token_replay"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .if_not_exists()
                    .table(ActorTokenReplay::Table)
                    .col(ColumnDef::new(ActorTokenReplay::Issuer).string().not_null())
                    .col(
                        ColumnDef::new(ActorTokenReplay::Audience)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ActorTokenReplay::Jti).string().not_null())
                    .col(
                        ColumnDef::new(ActorTokenReplay::IatUnix)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActorTokenReplay::ExpUnix)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ActorTokenReplay::ConsumedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(ActorTokenReplay::Issuer)
                            .col(ActorTokenReplay::Audience)
                            .col(ActorTokenReplay::Jti),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_actor_token_replay_exp_unix")
                    .table(ActorTokenReplay::Table)
                    .col(ActorTokenReplay::ExpUnix)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_actor_token_replay_exp_unix")
                    .table(ActorTokenReplay::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(Table::drop().table(ActorTokenReplay::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ActorTokenReplay {
    Table,
    Issuer,
    Audience,
    Jti,
    IatUnix,
    ExpUnix,
    ConsumedAt,
}
