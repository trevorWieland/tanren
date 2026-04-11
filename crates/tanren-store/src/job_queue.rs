//! [`JobQueue`] trait and the non-dequeue half of its implementation.
//!
//! Dequeue is special — it must be race-safe under concurrent workers,
//! and the SQL differs between backends — so it lives in a sibling
//! module, [`crate::job_queue_dequeue`]. Every other queue method is
//! a straightforward entity-API write, usually wrapped in a
//! transaction closure so its companion event append lands
//! co-transactionally.

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Statement,
    TransactionTrait,
};
use tanren_domain::{DispatchId, StepId, StepStatus};

use crate::converters::{events as event_converters, step as step_converters};
use crate::entity::{events, step_projection};
use crate::errors::{StoreError, StoreResult};
use crate::params::{
    AckAndEnqueueParams, DequeueParams, EnqueueStepParams, NackParams, QueuedStep,
};
use crate::store::Store;

/// Job queue / step lifecycle interface.
#[async_trait]
pub trait JobQueue: Send + Sync {
    /// Insert a new pending step. Appends the companion
    /// `StepEnqueued` envelope co-transactionally.
    async fn enqueue_step(&self, params: EnqueueStepParams) -> StoreResult<()>;

    /// Atomically claim the oldest pending step that matches the
    /// given lane filter, provided `running_count < max_concurrent`.
    /// Returns `None` if no candidate qualifies.
    async fn dequeue(&self, params: DequeueParams) -> StoreResult<Option<QueuedStep>>;

    /// Mark a step as completed, storing its result. Fails with
    /// [`StoreError::InvalidTransition`] if the step is not currently
    /// `running`.
    async fn ack(&self, step_id: &StepId, result: &tanren_domain::StepResult) -> StoreResult<()>;

    /// Single-transaction ack of the current step **and** enqueue of
    /// its successor. Both events (completion + optional next enqueue)
    /// are appended in the same transaction. This is the critical
    /// path the orchestrator uses to drive dispatches forward.
    async fn ack_and_enqueue(&self, params: AckAndEnqueueParams) -> StoreResult<()>;

    /// Cancel every pending non-teardown step belonging to a dispatch.
    /// Returns the number of rows updated.
    async fn cancel_pending_steps(&self, dispatch_id: &DispatchId) -> StoreResult<u64>;

    /// Mark a step as failed. If `params.retry` is true, the row is
    /// reset to `pending` with an incremented `retry_count` instead.
    /// Appends the companion `StepFailed` envelope co-transactionally.
    async fn nack(&self, step_id: &StepId, params: NackParams) -> StoreResult<()>;

    /// Reset steps that have been `running` longer than `timeout_secs`
    /// back to `pending`. Crash recovery for workers that died
    /// without releasing their claim. Returns the number of rows
    /// reset.
    async fn recover_stale_steps(&self, timeout_secs: u64) -> StoreResult<u64>;
}

#[async_trait]
impl JobQueue for Store {
    async fn enqueue_step(&self, params: EnqueueStepParams) -> StoreResult<()> {
        let now = Utc::now();
        let row = step_converters::enqueue_to_active_model(&params, now)?;
        let event_model = event_converters::envelope_to_active_model(&params.enqueue_event)?;
        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    step_projection::Entity::insert(row).exec(txn).await?;
                    events::Entity::insert(event_model).exec(txn).await?;
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }

    async fn dequeue(&self, params: DequeueParams) -> StoreResult<Option<QueuedStep>> {
        crate::job_queue_dequeue::dequeue_impl(self.conn(), params).await
    }

    async fn ack(&self, step_id: &StepId, result: &tanren_domain::StepResult) -> StoreResult<()> {
        let result_value = serde_json::to_value(result)?;
        let now = Utc::now();
        let update = step_projection::ActiveModel {
            step_id: Set(step_id.into_uuid()),
            status: Set(step_converters::step_status_to_string(StepStatus::Completed).to_owned()),
            result: Set(Some(result_value)),
            updated_at: Set(now),
            worker_id: Set(None),
            ..Default::default()
        };
        let outcome = step_projection::Entity::update(update)
            .filter(step_projection::Column::StepId.eq(step_id.into_uuid()))
            .filter(
                step_projection::Column::Status
                    .eq(step_converters::step_status_to_string(StepStatus::Running)),
            )
            .exec(self.conn())
            .await;
        match outcome {
            Ok(_) => Ok(()),
            Err(sea_orm::DbErr::RecordNotUpdated) => Err(StoreError::InvalidTransition {
                entity: format!("step {step_id}"),
                from: "running".to_owned(),
                to: "completed".to_owned(),
            }),
            Err(err) => Err(err.into()),
        }
    }

    async fn ack_and_enqueue(&self, params: AckAndEnqueueParams) -> StoreResult<()> {
        let now = Utc::now();
        let result_value = serde_json::to_value(&params.result)?;
        let completion_event =
            event_converters::envelope_to_active_model(&params.completion_event)?;
        let step_id_uuid = params.step_id.into_uuid();
        let step_id_display = params.step_id.to_string();

        let next = match params.next_step {
            Some(step) => {
                let row = step_converters::enqueue_to_active_model(&step, now)?;
                let event_model = event_converters::envelope_to_active_model(&step.enqueue_event)?;
                Some((row, event_model))
            }
            None => None,
        };

        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    // 1. Complete the current step. Require status='running'
                    //    via a filter so the update reports RecordNotUpdated
                    //    on any other state — a non-retryable bug.
                    let update = step_projection::ActiveModel {
                        step_id: Set(step_id_uuid),
                        status: Set(
                            step_converters::step_status_to_string(StepStatus::Completed)
                                .to_owned(),
                        ),
                        result: Set(Some(result_value)),
                        updated_at: Set(now),
                        worker_id: Set(None),
                        ..Default::default()
                    };
                    let result = step_projection::Entity::update(update)
                        .filter(step_projection::Column::StepId.eq(step_id_uuid))
                        .filter(
                            step_projection::Column::Status
                                .eq(step_converters::step_status_to_string(StepStatus::Running)),
                        )
                        .exec(txn)
                        .await;
                    match result {
                        Ok(_) => {}
                        Err(sea_orm::DbErr::RecordNotUpdated) => {
                            return Err(StoreError::InvalidTransition {
                                entity: format!("step {step_id_display}"),
                                from: "running".to_owned(),
                                to: "completed".to_owned(),
                            });
                        }
                        Err(err) => return Err(err.into()),
                    }

                    // 2. Append the completion envelope.
                    events::Entity::insert(completion_event).exec(txn).await?;

                    // 3. (Optional) Insert the successor step and
                    //    append its enqueue envelope.
                    if let Some((row, event_model)) = next {
                        step_projection::Entity::insert(row).exec(txn).await?;
                        events::Entity::insert(event_model).exec(txn).await?;
                    }

                    Ok(())
                })
            })
            .await?;
        Ok(())
    }

    async fn cancel_pending_steps(&self, dispatch_id: &DispatchId) -> StoreResult<u64> {
        let result = step_projection::Entity::update_many()
            .col_expr(
                step_projection::Column::Status,
                sea_orm::sea_query::Expr::value(step_converters::step_status_to_string(
                    StepStatus::Cancelled,
                )),
            )
            .col_expr(
                step_projection::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(step_projection::Column::DispatchId.eq(dispatch_id.into_uuid()))
            .filter(
                step_projection::Column::Status
                    .eq(step_converters::step_status_to_string(StepStatus::Pending)),
            )
            .filter(
                step_projection::Column::StepType.ne(step_converters::step_type_to_string(
                    tanren_domain::StepType::Teardown,
                )),
            )
            .exec(self.conn())
            .await?;
        Ok(result.rows_affected)
    }

    async fn nack(&self, step_id: &StepId, params: NackParams) -> StoreResult<()> {
        let now = Utc::now();
        let event_model = event_converters::envelope_to_active_model(&params.failure_event)?;
        let step_id_uuid = step_id.into_uuid();
        let error = params.error.clone();
        let retry = params.retry;

        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    let mut active = step_projection::ActiveModel {
                        step_id: Set(step_id_uuid),
                        updated_at: Set(now),
                        error: Set(Some(error)),
                        worker_id: Set(None),
                        ..Default::default()
                    };
                    if retry {
                        active.status =
                            Set(step_converters::step_status_to_string(StepStatus::Pending)
                                .to_owned());
                        // retry_count is bumped via raw SQL on the
                        // executing connection so we don't have to
                        // re-read the row to compute the new value.
                        txn.execute(Statement::from_sql_and_values(
                            txn.get_database_backend(),
                            "UPDATE step_projection SET retry_count = retry_count + 1 WHERE step_id = $1",
                            vec![step_id_uuid.into()],
                        ))
                        .await?;
                    } else {
                        active.status =
                            Set(step_converters::step_status_to_string(StepStatus::Failed)
                                .to_owned());
                    }
                    step_projection::Entity::update(active).exec(txn).await?;
                    events::Entity::insert(event_model).exec(txn).await?;
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }

    async fn recover_stale_steps(&self, timeout_secs: u64) -> StoreResult<u64> {
        let threshold = Utc::now()
            - chrono::Duration::try_seconds(i64::try_from(timeout_secs).unwrap_or(i64::MAX))
                .unwrap_or_else(chrono::Duration::zero);

        let result = step_projection::Entity::update_many()
            .col_expr(
                step_projection::Column::Status,
                sea_orm::sea_query::Expr::value(step_converters::step_status_to_string(
                    StepStatus::Pending,
                )),
            )
            .col_expr(
                step_projection::Column::WorkerId,
                sea_orm::sea_query::Expr::value(Option::<String>::None),
            )
            .col_expr(
                step_projection::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(
                step_projection::Column::Status
                    .eq(step_converters::step_status_to_string(StepStatus::Running)),
            )
            .filter(step_projection::Column::UpdatedAt.lt(threshold))
            .exec(self.conn())
            .await?;
        Ok(result.rows_affected)
    }
}
