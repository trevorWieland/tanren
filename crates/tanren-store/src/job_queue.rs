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
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};
use tanren_domain::{DispatchId, StepId, StepStatus, StepType};

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
    /// Fails with [`StoreError::InvalidTransition`] if the step is
    /// not currently `running`. Appends the companion `StepFailed`
    /// envelope co-transactionally.
    async fn nack(&self, step_id: &StepId, params: NackParams) -> StoreResult<()>;

    /// Refresh the worker liveness signal (`last_heartbeat_at`) on a
    /// running step. Workers call this periodically while executing
    /// long-running work so [`JobQueue::recover_stale_steps`] does
    /// not reclaim the step. Fails with
    /// [`StoreError::InvalidTransition`] if the step is not
    /// currently `running`.
    async fn heartbeat_step(&self, step_id: &StepId) -> StoreResult<()>;

    /// Reset `running` steps whose `last_heartbeat_at` is older than
    /// `timeout_secs` (or has never been refreshed) back to
    /// `pending`. Crash recovery for workers that died without
    /// releasing their claim. Live workers that call
    /// [`JobQueue::heartbeat_step`] within the threshold are never
    /// touched. Returns the number of rows reset.
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
            status: Set(StepStatus::Completed.to_string()),
            result: Set(Some(result_value)),
            updated_at: Set(now),
            worker_id: Set(None),
            ..Default::default()
        };
        let outcome = step_projection::Entity::update(update)
            .filter(step_projection::Column::StepId.eq(step_id.into_uuid()))
            .filter(step_projection::Column::Status.eq(StepStatus::Running.to_string()))
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
                    let update = step_projection::ActiveModel {
                        step_id: Set(step_id_uuid),
                        status: Set(StepStatus::Completed.to_string()),
                        result: Set(Some(result_value)),
                        updated_at: Set(now),
                        worker_id: Set(None),
                        ..Default::default()
                    };
                    let result = step_projection::Entity::update(update)
                        .filter(step_projection::Column::StepId.eq(step_id_uuid))
                        .filter(step_projection::Column::Status.eq(StepStatus::Running.to_string()))
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

                    events::Entity::insert(completion_event).exec(txn).await?;

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
                Expr::value(StepStatus::Cancelled.to_string()),
            )
            .col_expr(step_projection::Column::UpdatedAt, Expr::value(Utc::now()))
            .filter(step_projection::Column::DispatchId.eq(dispatch_id.into_uuid()))
            .filter(step_projection::Column::Status.eq(StepStatus::Pending.to_string()))
            .filter(step_projection::Column::StepType.ne(StepType::Teardown.to_string()))
            .exec(self.conn())
            .await?;
        Ok(result.rows_affected)
    }

    async fn nack(&self, step_id: &StepId, params: NackParams) -> StoreResult<()> {
        let now = Utc::now();
        let event_model = event_converters::envelope_to_active_model(&params.failure_event)?;
        let step_id_uuid = step_id.into_uuid();
        let step_id_display = step_id.to_string();
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
                        active.status = Set(StepStatus::Pending.to_string());
                    } else {
                        active.status = Set(StepStatus::Failed.to_string());
                    }
                    // Require `status = 'running'` so nack can only
                    // move a step out of the running state — domain
                    // rules (status/step.rs::can_transition_to) never
                    // allow Pending -> Failed directly.
                    let result = step_projection::Entity::update(active)
                        .filter(step_projection::Column::StepId.eq(step_id_uuid))
                        .filter(step_projection::Column::Status.eq(StepStatus::Running.to_string()))
                        .exec(txn)
                        .await;
                    match result {
                        Ok(_) => {}
                        Err(sea_orm::DbErr::RecordNotUpdated) => {
                            return Err(StoreError::InvalidTransition {
                                entity: format!("step {step_id_display}"),
                                from: "running".to_owned(),
                                to: if retry { "pending" } else { "failed" }.to_owned(),
                            });
                        }
                        Err(err) => return Err(err.into()),
                    }
                    if retry {
                        // Bump retry_count via the entity API. No raw
                        // SQL needed — Expr::col().add(1) compiles to
                        // `retry_count = retry_count + 1` on every
                        // backend, keeping the raw-SQL surface limited
                        // to the dequeue claim path (S-03).
                        step_projection::Entity::update_many()
                            .col_expr(
                                step_projection::Column::RetryCount,
                                Expr::col(step_projection::Column::RetryCount).add(1),
                            )
                            .filter(step_projection::Column::StepId.eq(step_id_uuid))
                            .exec(txn)
                            .await?;
                    }
                    events::Entity::insert(event_model).exec(txn).await?;
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }

    async fn heartbeat_step(&self, step_id: &StepId) -> StoreResult<()> {
        let now = Utc::now();
        let update = step_projection::ActiveModel {
            step_id: Set(step_id.into_uuid()),
            last_heartbeat_at: Set(Some(now)),
            ..Default::default()
        };
        let outcome = step_projection::Entity::update(update)
            .filter(step_projection::Column::StepId.eq(step_id.into_uuid()))
            .filter(step_projection::Column::Status.eq(StepStatus::Running.to_string()))
            .exec(self.conn())
            .await;
        match outcome {
            Ok(_) => Ok(()),
            Err(sea_orm::DbErr::RecordNotUpdated) => Err(StoreError::InvalidTransition {
                entity: format!("step {step_id}"),
                from: "running".to_owned(),
                to: "heartbeat".to_owned(),
            }),
            Err(err) => Err(err.into()),
        }
    }

    async fn recover_stale_steps(&self, timeout_secs: u64) -> StoreResult<u64> {
        let threshold = Utc::now()
            - chrono::Duration::try_seconds(i64::try_from(timeout_secs).unwrap_or(i64::MAX))
                .unwrap_or_else(chrono::Duration::zero);

        let result = step_projection::Entity::update_many()
            .col_expr(
                step_projection::Column::Status,
                Expr::value(StepStatus::Pending.to_string()),
            )
            .col_expr(
                step_projection::Column::WorkerId,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                step_projection::Column::LastHeartbeatAt,
                Expr::value(Option::<chrono::DateTime<Utc>>::None),
            )
            .col_expr(step_projection::Column::UpdatedAt, Expr::value(Utc::now()))
            .filter(step_projection::Column::Status.eq(StepStatus::Running.to_string()))
            // Only reclaim rows whose heartbeat is older than the
            // threshold. Rows whose heartbeat was refreshed within
            // the window stay running — liveness signal is
            // independent of `updated_at`, which is bumped by every
            // write and is not a reliable indicator of worker health.
            .filter(step_projection::Column::LastHeartbeatAt.lt(threshold))
            .exec(self.conn())
            .await?;
        Ok(result.rows_affected)
    }
}
