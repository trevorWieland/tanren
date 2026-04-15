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
use sea_orm::{
    ActiveValue::Set, ColumnTrait, Condition, EntityTrait, QueryFilter, TransactionTrait,
};
use tanren_domain::{
    DomainEvent, EntityKind, EventEnvelope, EventId, StepId, StepStatus, StepType,
};

use crate::converters::{events as event_converters, step as step_converters, validate};
use crate::entity::enums::{StepStatusModel, StepTypeModel};
use crate::entity::{dispatch_projection, events, step_projection};
use crate::errors::{StoreError, StoreResult};
use crate::params::{
    AckAndEnqueueParams, AckParams, CancelPendingStepsParams, DequeueParams, EnqueueStepParams,
    NackParams, QueuedStep,
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

    /// Mark a step as completed, storing its result. Appends the
    /// companion `StepCompleted` envelope co-transactionally. Fails
    /// with [`StoreError::InvalidTransition`] if the step is not
    /// currently `running`.
    async fn ack(&self, params: AckParams) -> StoreResult<()>;

    /// Single-transaction ack of the current step **and** enqueue of
    /// its successor. Both events (completion + optional next enqueue)
    /// are appended in the same transaction. This is the critical
    /// path the orchestrator uses to drive dispatches forward.
    async fn ack_and_enqueue(&self, params: AckAndEnqueueParams) -> StoreResult<()>;

    /// Cancel every pending non-teardown step belonging to a dispatch,
    /// generating one `StepCancelled` envelope per cancelled row and
    /// appending them all co-transactionally. Returns the number of
    /// rows updated.
    async fn cancel_pending_steps(&self, params: CancelPendingStepsParams) -> StoreResult<u64>;

    /// Mark a step as failed. If `params.retry` is true, the row is
    /// reset to `pending` with an incremented `retry_count` instead.
    /// Fails with [`StoreError::InvalidTransition`] if the step is
    /// not currently `running`. Appends the companion `StepFailed`
    /// envelope co-transactionally.
    async fn nack(&self, params: NackParams) -> StoreResult<()>;

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
        validate::validate_enqueue_step(&params)?;
        let now = Utc::now();
        let row = step_converters::enqueue_to_active_model(&params, now)?;
        let event_model = event_converters::envelope_to_active_model(&params.enqueue_event)?;
        let dispatch_uuid = params.dispatch_id.into_uuid();
        let dispatch_id_display = params.dispatch_id;
        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    // Verify the dispatch exists — application-level FK
                    // that protects both backends (SQLite FK is backed
                    // by PRAGMA foreign_keys, but belt-and-suspenders).
                    let exists = dispatch_projection::Entity::find_by_id(dispatch_uuid)
                        .one(txn)
                        .await?;
                    if exists.is_none() {
                        return Err(StoreError::NotFound {
                            entity_kind: EntityKind::Dispatch,
                            id: dispatch_id_display.to_string(),
                        });
                    }
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

    async fn ack(&self, params: AckParams) -> StoreResult<()> {
        validate::validate_ack(&params)?;

        let result_value = serde_json::to_value(&params.result)?;
        let event_model = event_converters::envelope_to_active_model(&params.completion_event)?;
        let now = Utc::now();
        let step_id = params.step_id;
        let step_id_uuid = step_id.into_uuid();
        let dispatch_id_uuid = params.dispatch_id.into_uuid();
        let step_type = StepTypeModel::from(params.step_type);

        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    let update = step_projection::ActiveModel {
                        step_id: Set(step_id_uuid),
                        status: Set(StepStatusModel::Completed),
                        result: Set(Some(result_value)),
                        error: Set(None),
                        updated_at: Set(now),
                        worker_id: Set(None),
                        ..Default::default()
                    };
                    let outcome = step_projection::Entity::update(update)
                        .filter(step_projection::Column::StepId.eq(step_id_uuid))
                        .filter(step_projection::Column::DispatchId.eq(dispatch_id_uuid))
                        .filter(step_projection::Column::Status.eq(StepStatusModel::Running))
                        .filter(step_projection::Column::StepType.eq(step_type))
                        .exec(txn)
                        .await;
                    match outcome {
                        Ok(_) => {}
                        Err(sea_orm::DbErr::RecordNotUpdated) => {
                            return Err(StoreError::InvalidTransition {
                                entity: format!("step {step_id}"),
                                from: "running".to_owned(),
                                to: "completed".to_owned(),
                            });
                        }
                        Err(err) => return Err(err.into()),
                    }
                    events::Entity::insert(event_model).exec(txn).await?;
                    Ok(())
                })
            })
            .await?;
        Ok(())
    }

    async fn ack_and_enqueue(&self, params: AckAndEnqueueParams) -> StoreResult<()> {
        validate::validate_ack_and_enqueue(&params)?;
        let now = Utc::now();
        let result_value = serde_json::to_value(&params.result)?;
        let completion_event =
            event_converters::envelope_to_active_model(&params.completion_event)?;
        let step_id_uuid = params.step_id.into_uuid();
        let dispatch_id_uuid = params.dispatch_id.into_uuid();
        let step_id_display = params.step_id.to_string();
        let step_type = StepTypeModel::from(params.step_type);

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
                        status: Set(StepStatusModel::Completed),
                        result: Set(Some(result_value)),
                        error: Set(None),
                        updated_at: Set(now),
                        worker_id: Set(None),
                        ..Default::default()
                    };
                    let result = step_projection::Entity::update(update)
                        .filter(step_projection::Column::StepId.eq(step_id_uuid))
                        .filter(step_projection::Column::DispatchId.eq(dispatch_id_uuid))
                        .filter(step_projection::Column::Status.eq(StepStatusModel::Running))
                        .filter(step_projection::Column::StepType.eq(step_type))
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

    async fn cancel_pending_steps(&self, params: CancelPendingStepsParams) -> StoreResult<u64> {
        let dispatch_id = params.dispatch_id;
        let actor = params.actor;
        let reason = params.reason;
        // Store-minted timestamp — not caller-controlled — so the
        // post-UPDATE SELECT key is unique to this operation.
        let timestamp = Utc::now();
        let dispatch_uuid = dispatch_id.into_uuid();

        self.conn()
            .transaction::<_, u64, StoreError>(move |txn| {
                Box::pin(async move {
                    // UPDATE first to atomically claim the rows.
                    // A concurrent dequeue that races this UPDATE
                    // will either see the row as still pending (and
                    // claim it before we run) or see it as cancelled
                    // (and skip it). Either way, the UPDATE's
                    // rows_affected count is authoritative.
                    let result = step_projection::Entity::update_many()
                        .col_expr(
                            step_projection::Column::Status,
                            Expr::value(StepStatusModel::Cancelled),
                        )
                        .col_expr(step_projection::Column::UpdatedAt, Expr::value(timestamp))
                        .filter(step_projection::Column::DispatchId.eq(dispatch_uuid))
                        .filter(step_projection::Column::Status.eq(StepStatusModel::Pending))
                        .filter(step_projection::Column::StepType.ne(StepTypeModel::Teardown))
                        .exec(txn)
                        .await?;

                    let count = result.rows_affected;
                    if count == 0 {
                        return Ok(0);
                    }

                    // Now SELECT the rows we actually cancelled to
                    // generate events. These rows are already
                    // status='cancelled' with our exact timestamp,
                    // so no TOCTOU window exists.
                    let rows = step_projection::Entity::find()
                        .filter(step_projection::Column::DispatchId.eq(dispatch_uuid))
                        .filter(step_projection::Column::Status.eq(StepStatusModel::Cancelled))
                        .filter(step_projection::Column::UpdatedAt.eq(timestamp))
                        .filter(step_projection::Column::StepType.ne(StepTypeModel::Teardown))
                        .all(txn)
                        .await?;

                    let mut event_models = Vec::with_capacity(rows.len());
                    for row in &rows {
                        let envelope = mint_step_cancelled(
                            dispatch_id,
                            row,
                            actor.as_ref(),
                            reason.as_ref(),
                            timestamp,
                        )?;
                        event_models.push(envelope);
                    }
                    if !event_models.is_empty() {
                        events::Entity::insert_many(event_models).exec(txn).await?;
                    }

                    Ok(count)
                })
            })
            .await
            .map_err(StoreError::from)
    }

    async fn nack(&self, params: NackParams) -> StoreResult<()> {
        validate::validate_nack(&params)?;
        let now = Utc::now();
        let event_model = event_converters::envelope_to_active_model(&params.failure_event)?;
        let step_id_uuid = params.step_id.into_uuid();
        let dispatch_id_uuid = params.dispatch_id.into_uuid();
        let step_id_display = params.step_id.to_string();
        let step_type = StepTypeModel::from(params.step_type);
        let error = params.error.clone();
        let retry = params.retry;

        self.conn()
            .transaction::<_, (), StoreError>(move |txn| {
                Box::pin(async move {
                    let (status, error_value) = if retry {
                        (StepStatus::Pending, None)
                    } else {
                        (StepStatus::Failed, Some(error))
                    };
                    let active = step_projection::ActiveModel {
                        step_id: Set(step_id_uuid),
                        status: Set(StepStatusModel::from(status)),
                        updated_at: Set(now),
                        error: Set(error_value),
                        worker_id: Set(None),
                        ..Default::default()
                    };
                    // Require `status = 'running'` so nack can only
                    // move a step out of the running state — domain
                    // rules (status/step.rs::can_transition_to) never
                    // allow Pending -> Failed directly.
                    let result = step_projection::Entity::update(active)
                        .filter(step_projection::Column::StepId.eq(step_id_uuid))
                        .filter(step_projection::Column::DispatchId.eq(dispatch_id_uuid))
                        .filter(step_projection::Column::Status.eq(StepStatusModel::Running))
                        .filter(step_projection::Column::StepType.eq(step_type))
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
            .filter(step_projection::Column::Status.eq(StepStatusModel::Running))
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
                Expr::value(StepStatusModel::Pending),
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
            .filter(step_projection::Column::Status.eq(StepStatusModel::Running))
            // Reclaim rows whose heartbeat is older than the
            // threshold OR has never been set (NULL). Rows whose
            // heartbeat was refreshed within the window stay
            // running — liveness signal is independent of
            // `updated_at`, which is bumped by every write and is
            // not a reliable indicator of worker health.
            .filter(
                Condition::any()
                    .add(step_projection::Column::LastHeartbeatAt.lt(threshold))
                    .add(step_projection::Column::LastHeartbeatAt.is_null()),
            )
            .exec(self.conn())
            .await?;
        Ok(result.rows_affected)
    }
}

/// Build a `StepCancelled` event active model from a cancelled
/// projection row + caller metadata.
fn mint_step_cancelled(
    dispatch_id: tanren_domain::DispatchId,
    row: &step_projection::Model,
    actor: Option<&tanren_domain::ActorContext>,
    reason: Option<&String>,
    timestamp: chrono::DateTime<Utc>,
) -> Result<events::ActiveModel, StoreError> {
    let step_id = StepId::from_uuid(row.step_id);
    let step_type = StepType::from(row.step_type);
    let envelope = EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        timestamp,
        DomainEvent::StepCancelled {
            dispatch_id,
            step_id,
            step_type,
            caused_by: actor.cloned(),
            reason: reason.cloned(),
        },
    );
    event_converters::envelope_to_active_model(&envelope)
}
