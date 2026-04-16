//! Cancel-path indexes for large dispatch fan-outs.
//!
//! Supports bounded batched cancellation predicates:
//! `dispatch_id = ? AND status = 'pending' AND step_type != 'teardown'`
//! with deterministic chunk ordering by `step_id`.

use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0005_cancel_dispatch_indexes"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("ix_step_cancel_dispatch_pending")
                    .table(StepProjection::Table)
                    .col(StepProjection::DispatchId)
                    .col(StepProjection::Status)
                    .col(StepProjection::StepType)
                    .col(StepProjection::StepId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("ix_step_cancel_dispatch_pending")
                    .table(StepProjection::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum StepProjection {
    Table,
    DispatchId,
    Status,
    StepType,
    StepId,
}
