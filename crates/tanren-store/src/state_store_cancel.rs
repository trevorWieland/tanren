use std::time::Duration;

use chrono::Utc;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, DbBackend, DbErr, EntityTrait, QueryFilter,
    Statement, sea_query::Expr,
};
use tanren_domain::{
    ActorContext, DispatchId, DispatchStatus, DomainEvent, EntityKind, EventEnvelope, EventId,
    StepId, StepType,
};

use crate::converters::events as event_converters;
use crate::db_error_codes::{
    extract_db_error_code, is_postgres_contention_code, is_sqlite_contention_code,
};
use crate::entity::enums::{DispatchStatusModel, OutcomeModel, StepTypeModel};
use crate::entity::{dispatch_projection, events};
use crate::errors::{StoreConflictClass, StoreError, StoreOperation};
use crate::params::ReplayGuard;
use crate::sql_tags::{
    STATUS_CANCELLED as STEP_STATUS_CANCELLED, STATUS_PENDING as STEP_STATUS_PENDING,
    STEP_TYPE_TEARDOWN,
};
use crate::token_replay_store::consume_replay_guard_once;

const CANCEL_BATCH_SIZE: u64 = 500;
/// Hard cap on how many `cancel` batches a single transaction may
/// issue. Bounds the worst case where every batch returns the
/// maximum row count: `MAX_CANCEL_OUTER_BATCHES * CANCEL_BATCH_SIZE`
/// is the largest dispatch we will ever fully cancel in one txn.
const MAX_CANCEL_OUTER_BATCHES: u32 = 1_024;
/// Maximum times we will retry an *empty* `SKIP LOCKED` batch when
/// pending rows still exist (i.e. they are merely locked by a
/// concurrent dequeue). After this many consecutive empty-but-not-
/// drained iterations, we surface a typed contention conflict so the
/// caller can decide whether to retry the whole cancel.
const MAX_CANCEL_LOCK_RETRIES: u32 = 8;
/// Base back-off between lock-contention retries. Doubles each
/// retry, capped at [`LOCK_RETRY_CAP`]. With base 5 ms and cap
/// 1 s the worst-case wall-clock budget is roughly 1.3 s.
const LOCK_RETRY_BASE: Duration = Duration::from_millis(5);
const LOCK_RETRY_CAP: Duration = Duration::from_secs(1);

pub(crate) struct CancelDispatchTxnInput {
    pub(crate) dispatch_id: DispatchId,
    pub(crate) dispatch_uuid: uuid::Uuid,
    pub(crate) actor: ActorContext,
    pub(crate) reason: Option<String>,
    pub(crate) replay_guard: ReplayGuard,
    pub(crate) now: chrono::DateTime<Utc>,
    pub(crate) step_event_timestamp: chrono::DateTime<Utc>,
    pub(crate) dispatch_event_model: events::ActiveModel,
}

pub(crate) async fn run_cancel_dispatch_transaction(
    txn: &DatabaseTransaction,
    input: CancelDispatchTxnInput,
) -> Result<u64, StoreError> {
    let CancelDispatchTxnInput {
        dispatch_id,
        dispatch_uuid,
        actor,
        reason,
        replay_guard,
        now,
        step_event_timestamp,
        dispatch_event_model,
    } = input;

    consume_replay_guard_once(txn, replay_guard).await?;

    let current = fetch_dispatch_status_for_cancel(txn, dispatch_id, dispatch_uuid).await?;
    let cancelled_count = cancel_pending_steps_and_emit_events(
        txn,
        dispatch_id,
        dispatch_uuid,
        &actor,
        reason.as_ref(),
        now,
        step_event_timestamp,
    )
    .await?;

    let result = dispatch_projection::Entity::update_many()
        .col_expr(
            dispatch_projection::Column::Status,
            Expr::value(DispatchStatusModel::Cancelled),
        )
        .col_expr(
            dispatch_projection::Column::Outcome,
            Expr::value(Option::<OutcomeModel>::None),
        )
        .col_expr(dispatch_projection::Column::UpdatedAt, Expr::value(now))
        .filter(dispatch_projection::Column::DispatchId.eq(dispatch_uuid))
        .filter(dispatch_projection::Column::Status.eq(DispatchStatusModel::from(current)))
        .exec(txn)
        .await?;
    if result.rows_affected == 0 {
        return Err(StoreError::Conflict {
            class: StoreConflictClass::Contention,
            operation: StoreOperation::CancelDispatch,
            reason: format!("dispatch {dispatch_id} status changed concurrently from {current}"),
        });
    }

    events::Entity::insert(dispatch_event_model)
        .exec(txn)
        .await?;
    Ok(cancelled_count)
}

pub(crate) fn normalize_cancel_error(err: StoreError) -> StoreError {
    match err {
        StoreError::Database(db_err) => {
            classify_contention_db_error(&db_err).map_or(StoreError::Database(db_err), |class| {
                StoreError::Conflict {
                    class,
                    operation: StoreOperation::CancelDispatch,
                    reason: "dispatch cancellation contention".to_owned(),
                }
            })
        }
        other => other,
    }
}

async fn fetch_dispatch_status_for_cancel(
    txn: &DatabaseTransaction,
    dispatch_id: DispatchId,
    dispatch_uuid: uuid::Uuid,
) -> Result<DispatchStatus, StoreError> {
    let row = dispatch_projection::Entity::find_by_id(dispatch_uuid)
        .one(txn)
        .await?;
    let row = row.ok_or_else(|| StoreError::NotFound {
        entity_kind: EntityKind::Dispatch,
        id: dispatch_id.to_string(),
    })?;

    let current = DispatchStatus::from(row.status);
    if !current.can_transition_to(DispatchStatus::Cancelled) {
        return Err(StoreError::InvalidTransition {
            entity: format!("dispatch {dispatch_id}"),
            from: current.to_string(),
            to: DispatchStatus::Cancelled.to_string(),
        });
    }
    Ok(current)
}

async fn cancel_pending_steps_and_emit_events(
    txn: &DatabaseTransaction,
    dispatch_id: DispatchId,
    dispatch_uuid: uuid::Uuid,
    actor: &ActorContext,
    reason: Option<&String>,
    now: chrono::DateTime<Utc>,
    step_event_timestamp: chrono::DateTime<Utc>,
) -> Result<u64, StoreError> {
    let backend = txn.get_database_backend();
    let mut total_cancelled = 0_u64;
    // `SKIP LOCKED` (Postgres path) returns an empty result both
    // when there are genuinely no eligible rows AND when every
    // candidate row is currently locked by a concurrent dequeue.
    // Treating "empty batch" as "done" silently skips locked rows
    // and leaves a half-cancelled dispatch behind. Track empty
    // batches and verify with a separate predicate before exiting.
    let mut lock_retries = 0_u32;

    for _ in 0..MAX_CANCEL_OUTER_BATCHES {
        let rows = cancel_pending_steps_batch_returning(backend, txn, dispatch_uuid, now).await?;
        if !rows.is_empty() {
            let mut events_to_insert = Vec::with_capacity(rows.len());
            for (step_id_uuid, step_type_model) in &rows {
                events_to_insert.push(mint_step_cancelled(
                    dispatch_id,
                    *step_id_uuid,
                    *step_type_model,
                    actor,
                    reason,
                    step_event_timestamp,
                )?);
            }
            events::Entity::insert_many(events_to_insert)
                .exec(txn)
                .await?;
            total_cancelled = total_cancelled.saturating_add(rows.len() as u64);
            // Progress was made; reset the lock-retry budget.
            lock_retries = 0;
            continue;
        }

        // Empty batch — distinguish "drained" from "locked".
        if !pending_non_teardown_steps_exist(backend, txn, dispatch_uuid).await? {
            return Ok(total_cancelled);
        }

        lock_retries = lock_retries.saturating_add(1);
        if lock_retries >= MAX_CANCEL_LOCK_RETRIES {
            return Err(StoreError::Conflict {
                class: StoreConflictClass::Contention,
                operation: StoreOperation::CancelDispatch,
                reason: format!(
                    "dispatch {dispatch_id}: pending steps remained locked across {MAX_CANCEL_LOCK_RETRIES} cancel retries"
                ),
            });
        }
        tokio::time::sleep(lock_backoff(lock_retries)).await;
    }

    // The outer loop should never exhaust under normal operation
    // — `MAX_CANCEL_OUTER_BATCHES * CANCEL_BATCH_SIZE` is far above
    // any realistic dispatch graph. Hitting it indicates either a
    // pathological dispatch shape or a logic regression; surface
    // it as typed contention rather than silently returning a
    // partial count.
    Err(StoreError::Conflict {
        class: StoreConflictClass::Contention,
        operation: StoreOperation::CancelDispatch,
        reason: format!(
            "dispatch {dispatch_id}: cancel loop exceeded {MAX_CANCEL_OUTER_BATCHES} batches"
        ),
    })
}

fn lock_backoff(retry: u32) -> Duration {
    // 5ms, 10ms, 20ms, … capped at LOCK_RETRY_CAP.
    let shift = retry.saturating_sub(1).min(20);
    let scaled = LOCK_RETRY_BASE
        .checked_mul(1u32.checked_shl(shift).unwrap_or(u32::MAX))
        .unwrap_or(LOCK_RETRY_CAP);
    scaled.min(LOCK_RETRY_CAP)
}

async fn pending_non_teardown_steps_exist(
    backend: DbBackend,
    txn: &DatabaseTransaction,
    dispatch_uuid: uuid::Uuid,
) -> Result<bool, StoreError> {
    // Plain `SELECT` (no `FOR UPDATE`) — we only need to know
    // whether any matching row is visible at all, including rows
    // currently row-locked by another transaction. In Postgres
    // READ COMMITTED, plain reads see all committed data and are
    // not blocked by row-level locks held by other writers, so
    // this returns `true` exactly when an eligible cancellable
    // row exists somewhere in the table.
    let stmt = match backend {
        DbBackend::Postgres => Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT 1 AS one FROM step_projection \
             WHERE dispatch_id = $1 AND status = $2 AND step_type <> $3 \
             LIMIT 1",
            vec![
                dispatch_uuid.into(),
                STEP_STATUS_PENDING.into(),
                STEP_TYPE_TEARDOWN.into(),
            ],
        ),
        DbBackend::Sqlite => Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT 1 AS one FROM step_projection \
             WHERE dispatch_id = ? AND status = ? AND step_type <> ? \
             LIMIT 1",
            vec![
                dispatch_uuid.into(),
                STEP_STATUS_PENDING.into(),
                STEP_TYPE_TEARDOWN.into(),
            ],
        ),
        DbBackend::MySql => {
            return Err(StoreError::Conversion {
                context: "state_store_cancel::pending_non_teardown_steps_exist",
                reason: "MySQL is not a supported backend".to_owned(),
            });
        }
    };
    Ok(txn.query_one(stmt).await?.is_some())
}

/// Cancel one batch of pending non-teardown steps for a dispatch and
/// return the affected `(step_id, step_type)` tuples in a single
/// round trip via `UPDATE ... RETURNING`.
///
/// Backend dispatch chooses between a Postgres variant using
/// `FOR UPDATE SKIP LOCKED` in the selector subquery (so concurrent
/// dequeue races yield to the cancel transaction) and a `SQLite` variant
/// that relies on `BEGIN IMMEDIATE` write-lock semantics.
async fn cancel_pending_steps_batch_returning(
    backend: DbBackend,
    txn: &DatabaseTransaction,
    dispatch_uuid: uuid::Uuid,
    now: chrono::DateTime<Utc>,
) -> Result<Vec<(uuid::Uuid, StepTypeModel)>, StoreError> {
    let limit = i64::try_from(CANCEL_BATCH_SIZE).map_err(|_| StoreError::Conversion {
        context: "state_store_cancel::cancel_pending_steps_batch_returning",
        reason: "CANCEL_BATCH_SIZE exceeds i64::MAX".to_owned(),
    })?;

    let stmt = match backend {
        DbBackend::Postgres => postgres_cancel_batch_returning_statement(dispatch_uuid, now, limit),
        DbBackend::Sqlite => sqlite_cancel_batch_returning_statement(dispatch_uuid, now, limit),
        DbBackend::MySql => {
            return Err(StoreError::Conversion {
                context: "state_store_cancel::cancel_pending_steps_batch_returning",
                reason: "MySQL is not a supported backend".to_owned(),
            });
        }
    };

    let rows = txn.query_all(stmt).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let step_id: uuid::Uuid = row.try_get("", "step_id")?;
        let step_type_raw: String = row.try_get("", "step_type")?;
        let step_type_model = parse_step_type_model(&step_type_raw)?;
        out.push((step_id, step_type_model));
    }
    Ok(out)
}

fn postgres_cancel_batch_returning_statement(
    dispatch_uuid: uuid::Uuid,
    now: chrono::DateTime<Utc>,
    limit: i64,
) -> Statement {
    const SQL: &str = "UPDATE step_projection \
         SET status = $1, updated_at = $2 \
         WHERE dispatch_id = $3 \
           AND status = $4 \
           AND step_type <> $5 \
           AND step_id IN ( \
             SELECT step_id FROM step_projection \
             WHERE dispatch_id = $3 \
               AND status = $4 \
               AND step_type <> $5 \
             ORDER BY step_id \
             LIMIT $6 \
             FOR UPDATE SKIP LOCKED \
           ) \
         RETURNING step_id, step_type";
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        SQL,
        vec![
            STEP_STATUS_CANCELLED.into(),
            now.into(),
            dispatch_uuid.into(),
            STEP_STATUS_PENDING.into(),
            STEP_TYPE_TEARDOWN.into(),
            limit.into(),
        ],
    )
}

fn sqlite_cancel_batch_returning_statement(
    dispatch_uuid: uuid::Uuid,
    now: chrono::DateTime<Utc>,
    limit: i64,
) -> Statement {
    // SQLite 3.35+ supports RETURNING. BEGIN IMMEDIATE held by the
    // surrounding txn serializes writers, so no FOR UPDATE equivalent
    // is needed here.
    const SQL: &str = "UPDATE step_projection \
         SET status = ?, updated_at = ? \
         WHERE dispatch_id = ? \
           AND status = ? \
           AND step_type <> ? \
           AND step_id IN ( \
             SELECT step_id FROM step_projection \
             WHERE dispatch_id = ? \
               AND status = ? \
               AND step_type <> ? \
             ORDER BY step_id \
             LIMIT ? \
           ) \
         RETURNING step_id, step_type";
    Statement::from_sql_and_values(
        DbBackend::Sqlite,
        SQL,
        vec![
            STEP_STATUS_CANCELLED.into(),
            now.into(),
            dispatch_uuid.into(),
            STEP_STATUS_PENDING.into(),
            STEP_TYPE_TEARDOWN.into(),
            dispatch_uuid.into(),
            STEP_STATUS_PENDING.into(),
            STEP_TYPE_TEARDOWN.into(),
            limit.into(),
        ],
    )
}

fn parse_step_type_model(raw: &str) -> Result<StepTypeModel, StoreError> {
    match raw {
        "provision" => Ok(StepTypeModel::Provision),
        "execute" => Ok(StepTypeModel::Execute),
        "teardown" => Ok(StepTypeModel::Teardown),
        "dry_run" => Ok(StepTypeModel::DryRun),
        other => Err(StoreError::Conversion {
            context: "state_store_cancel::parse_step_type_model",
            reason: format!("unrecognized step_type string `{other}` in RETURNING"),
        }),
    }
}

fn mint_step_cancelled(
    dispatch_id: DispatchId,
    step_id_uuid: uuid::Uuid,
    step_type_model: StepTypeModel,
    actor: &ActorContext,
    reason: Option<&String>,
    timestamp: chrono::DateTime<Utc>,
) -> Result<events::ActiveModel, StoreError> {
    let step_id = StepId::from_uuid(step_id_uuid);
    let step_type = StepType::from(step_type_model);
    let envelope = EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        timestamp,
        DomainEvent::StepCancelled {
            dispatch_id,
            step_id,
            step_type,
            caused_by: Some(actor.clone()),
            reason: reason.cloned(),
        },
    );
    event_converters::envelope_to_active_model(&envelope)
}

fn classify_contention_db_error(db_err: &DbErr) -> Option<StoreConflictClass> {
    let code = extract_db_error_code(db_err)?;
    if is_sqlite_contention_code(&code) || is_postgres_contention_code(&code) {
        Some(StoreConflictClass::Contention)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::DbErr;

    use super::normalize_cancel_error;
    use crate::StoreError;
    use crate::db_error_codes::{is_postgres_contention_code, is_sqlite_contention_code};

    #[test]
    fn sqlite_contention_codes_detect_busy_and_locked_classes() {
        assert!(
            is_sqlite_contention_code("5"),
            "SQLITE_BUSY should classify"
        );
        assert!(
            is_sqlite_contention_code("6"),
            "SQLITE_LOCKED should classify"
        );
        assert!(
            is_sqlite_contention_code("261"),
            "extended SQLITE_BUSY code should classify via primary code"
        );
        assert!(
            !is_sqlite_contention_code("2067"),
            "unique violation is not contention"
        );
    }

    #[test]
    fn postgres_contention_codes_detect_deadlock_and_serialization() {
        assert!(is_postgres_contention_code("40P01"));
        assert!(is_postgres_contention_code("40001"));
        assert!(is_postgres_contention_code("55P03"));
        assert!(!is_postgres_contention_code("23505"));
    }

    #[test]
    fn normalize_cancel_error_preserves_non_typed_database_errors() {
        let err = StoreError::Database(DbErr::Custom("connection refused".to_owned()));
        let normalized = normalize_cancel_error(err);
        assert!(
            matches!(normalized, StoreError::Database(_)),
            "expected database error, got {normalized:?}"
        );
    }
}
