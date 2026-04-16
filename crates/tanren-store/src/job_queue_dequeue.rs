//! Atomic dequeue — the one place raw SQL remains.
//!
//! `dequeue` must perform the "check running-count, claim pending
//! candidate" decision in a single atomic step. `SeaORM`'s entity
//! API cannot express this, so we branch on `DbBackend`.
//!
//! **Postgres**: hierarchical advisory locks serialize dequeue by
//! lane scope. `lane=None` takes an exclusive lock on global key
//! `(42, 0)`; `lane=Some(L)` takes exclusive on the lane key plus
//! shared on global, so different lanes dequeue in parallel.
//!
//! **`SQLite`**: `BEGIN IMMEDIATE` + `busy_timeout` (set in
//! `connection.rs`). We upgrade from `SeaORM`'s `BEGIN DEFERRED` via
//! `END; BEGIN IMMEDIATE` on the same sticky connection.
//!
//! Both backends split claim SQL into lane-scoped and global
//! variants so the query planner can use the `(lane, status)`
//! composite index instead of the `IS NULL OR` disjunction.

use chrono::Utc;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement, TransactionTrait, Value,
};
use tanren_domain::{DomainEvent, EventEnvelope, EventId};

use crate::converters::{events as event_converters, step as step_converters};
use crate::entity::{events, step_projection};
use crate::errors::{StoreError, StoreResult};
use crate::params::{DequeueParams, QueuedStep};

/// Implementation entry point called from
/// [`JobQueue::dequeue`](crate::JobQueue::dequeue).
pub(crate) async fn dequeue_impl(
    conn: &DatabaseConnection,
    params: DequeueParams,
) -> StoreResult<Option<QueuedStep>> {
    let max_concurrent =
        i64::try_from(params.max_concurrent).map_err(|_| StoreError::Conversion {
            context: "job_queue_dequeue::dequeue_impl",
            reason: "max_concurrent exceeds i64::MAX".to_owned(),
        })?;
    let lane_typed = params.lane;
    let lane_string = lane_typed.map(|l| l.to_string());
    let worker_id = params.worker_id;

    let backend = conn.get_database_backend();
    match backend {
        DbBackend::Postgres => {
            dequeue_postgres(conn, worker_id, lane_string, lane_typed, max_concurrent).await
        }
        DbBackend::Sqlite => dequeue_sqlite(conn, worker_id, lane_string, max_concurrent).await,
        DbBackend::MySql => Err(StoreError::Conversion {
            context: "job_queue_dequeue::dequeue_impl",
            reason: "MySQL is not a supported backend".to_owned(),
        }),
    }
}

/// Fixed namespace for `pg_advisory_xact_lock` calls made by the
/// store. Prevents accidental collisions with advisory locks taken
/// by unrelated subsystems.
const ADVISORY_LOCK_NAMESPACE: i32 = 42;

/// Global dequeue key. `lane=None` takes an exclusive lock on this
/// key; `lane=Some(L)` takes a shared lock on it (plus an exclusive
/// lock on the lane-specific key). This ensures `lane=None` blocks
/// all lane-specific dequeues, but different lanes can proceed in
/// parallel.
const ADVISORY_LOCK_GLOBAL: i32 = 0;

// Domain-enum string literals for raw SQL; drift is caught at
// compile time via the exhaustive-match guard in `crate::sql_tags`.
use crate::sql_tags::{READY_STATE_READY, STATUS_PENDING, STATUS_RUNNING};

const RETURNING_COLS: &str = "\
RETURNING step_id, dispatch_id, step_type, step_sequence, lane, status, \
ready_state, depends_on, graph_revision, worker_id, payload, \
result, error, retry_count, last_heartbeat_at, created_at, updated_at";

/// Map a lane to a unique advisory-lock key.
fn lane_advisory_key(lane: tanren_domain::Lane) -> i32 {
    match lane {
        tanren_domain::Lane::Impl => 1,
        tanren_domain::Lane::Audit => 2,
        tanren_domain::Lane::Gate => 3,
    }
}

async fn dequeue_postgres(
    conn: &DatabaseConnection,
    worker_id: String,
    lane: Option<String>,
    lane_typed: Option<tanren_domain::Lane>,
    max_concurrent: i64,
) -> StoreResult<Option<QueuedStep>> {
    conn.transaction::<_, Option<QueuedStep>, StoreError>(move |txn| {
        Box::pin(async move {
            // Hierarchical advisory locks:
            // - lane=None  -> exclusive on global key
            // - lane=Some  -> exclusive on lane key, shared on global
            match lane_typed {
                None => {
                    txn.execute(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT pg_advisory_xact_lock($1, $2)",
                        vec![ADVISORY_LOCK_NAMESPACE.into(), ADVISORY_LOCK_GLOBAL.into()],
                    ))
                    .await?;
                }
                Some(l) => {
                    let lane_key = lane_advisory_key(l);
                    txn.execute(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT pg_advisory_xact_lock($1, $2)",
                        vec![ADVISORY_LOCK_NAMESPACE.into(), lane_key.into()],
                    ))
                    .await?;
                    txn.execute(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        "SELECT pg_advisory_xact_lock_shared($1, $2)",
                        vec![ADVISORY_LOCK_NAMESPACE.into(), ADVISORY_LOCK_GLOBAL.into()],
                    ))
                    .await?;
                }
            }

            let stmt = postgres_claim_statement(&worker_id, lane, max_concurrent);
            let row = step_projection::Entity::find()
                .from_raw_sql(stmt)
                .one(txn)
                .await?;
            match row {
                Some(model) => {
                    let queued = step_converters::model_to_queued_step(model)?;
                    let dequeued_event =
                        mint_step_dequeued(queued.dispatch_id, queued.step_id, &worker_id)?;
                    events::Entity::insert(dequeued_event).exec(txn).await?;
                    Ok(Some(queued))
                }
                None => Ok(None),
            }
        })
    })
    .await
    .map_err(StoreError::from)
}

/// `SQLite` dequeue under `BEGIN IMMEDIATE`.
///
/// `SeaORM`'s `conn.transaction()` always issues `BEGIN DEFERRED`,
/// so we acquire a sticky connection via `conn.begin()` and
/// immediately upgrade to `IMMEDIATE` by ending the deferred
/// transaction and starting a new immediate one on the same handle.
/// This satisfies the audit brief's requirement and ensures the
/// write lock is held from the very start of the transaction.
async fn dequeue_sqlite(
    conn: &DatabaseConnection,
    worker_id: String,
    lane: Option<String>,
    max_concurrent: i64,
) -> StoreResult<Option<QueuedStep>> {
    // `conn.begin()` acquires a sticky pooled connection and issues
    // `BEGIN` (deferred). We hold this handle for the lifetime of
    // the dequeue so every subsequent statement runs on the same
    // connection.
    let txn = conn.begin().await?;

    // Upgrade: end the deferred transaction and immediately start
    // an IMMEDIATE one. Both statements execute on the sticky
    // connection held by `txn`. If `BEGIN IMMEDIATE` fails (e.g.,
    // `SQLITE_BUSY` after busy_timeout exhaustion), the error
    // propagates and `txn`'s drop handler is a no-op (no active
    // transaction to rollback — we already ended it).
    txn.execute_unprepared("END").await?;
    txn.execute_unprepared("BEGIN IMMEDIATE").await?;

    let stmt = sqlite_claim_statement(&worker_id, lane, max_concurrent);
    let result = step_projection::Entity::find()
        .from_raw_sql(stmt)
        .one(&txn)
        .await;

    match result {
        Ok(Some(model)) => {
            let queued = step_converters::model_to_queued_step(model)?;
            let dequeued_event =
                mint_step_dequeued(queued.dispatch_id, queued.step_id, &worker_id)?;
            events::Entity::insert(dequeued_event).exec(&txn).await?;
            txn.commit().await?;
            Ok(Some(queued))
        }
        Ok(None) => {
            txn.commit().await?;
            Ok(None)
        }
        Err(err) => {
            txn.rollback().await?;
            Err(err.into())
        }
    }
}

/// Build a `StepDequeued` event [`events::ActiveModel`] for the
/// claimed row.
fn mint_step_dequeued(
    dispatch_id: tanren_domain::DispatchId,
    step_id: tanren_domain::StepId,
    worker_id: &str,
) -> Result<events::ActiveModel, StoreError> {
    let envelope = EventEnvelope::new(
        EventId::from_uuid(uuid::Uuid::now_v7()),
        Utc::now(),
        DomainEvent::StepDequeued {
            dispatch_id,
            step_id,
            worker_id: worker_id.to_owned(),
        },
    );
    event_converters::envelope_to_active_model(&envelope)
}

/// Build the `Postgres` `UPDATE...RETURNING` claim statement.
pub(crate) fn postgres_claim_statement(
    worker_id: &str,
    lane: Option<String>,
    max_concurrent: i64,
) -> Statement {
    match lane {
        Some(l) => postgres_claim_lane(worker_id, l, max_concurrent),
        None => postgres_claim_global(worker_id, max_concurrent),
    }
}

fn postgres_claim_lane(worker_id: &str, lane: String, max_concurrent: i64) -> Statement {
    let sql = format!(
        "UPDATE step_projection
        SET status = '{STATUS_RUNNING}',
            worker_id = $1,
            updated_at = NOW(),
            last_heartbeat_at = NOW()
        WHERE step_id = (
            SELECT step_id FROM step_projection
            WHERE status = '{STATUS_PENDING}'
              AND ready_state = '{READY_STATE_READY}'
              AND lane = $2
              AND (
                  SELECT COUNT(*) FROM step_projection
                  WHERE status = '{STATUS_RUNNING}'
                    AND lane = $2
              ) < $3
            ORDER BY created_at ASC, step_sequence ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        {RETURNING_COLS}"
    );
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        vec![
            Value::from(worker_id.to_owned()),
            Value::from(lane),
            Value::from(max_concurrent),
        ],
    )
}

fn postgres_claim_global(worker_id: &str, max_concurrent: i64) -> Statement {
    let sql = format!(
        "UPDATE step_projection
        SET status = '{STATUS_RUNNING}',
            worker_id = $1,
            updated_at = NOW(),
            last_heartbeat_at = NOW()
        WHERE step_id = (
            SELECT step_id FROM step_projection
            WHERE status = '{STATUS_PENDING}'
              AND ready_state = '{READY_STATE_READY}'
              AND (
                  SELECT COUNT(*) FROM step_projection
                  WHERE status = '{STATUS_RUNNING}'
              ) < $2
            ORDER BY created_at ASC, step_sequence ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        {RETURNING_COLS}"
    );
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        sql,
        vec![
            Value::from(worker_id.to_owned()),
            Value::from(max_concurrent),
        ],
    )
}

/// Build the `SQLite` `UPDATE ... RETURNING` claim statement.
pub(crate) fn sqlite_claim_statement(
    worker_id: &str,
    lane: Option<String>,
    max_concurrent: i64,
) -> Statement {
    match lane {
        Some(l) => sqlite_claim_lane(worker_id, l, max_concurrent),
        None => sqlite_claim_global(worker_id, max_concurrent),
    }
}

fn sqlite_claim_lane(worker_id: &str, lane: String, max_concurrent: i64) -> Statement {
    let now = Utc::now();
    let sql = format!(
        "UPDATE step_projection
        SET status = '{STATUS_RUNNING}',
            worker_id = ?1,
            updated_at = ?2,
            last_heartbeat_at = ?2
        WHERE step_id = (
            SELECT step_id FROM step_projection
            WHERE status = '{STATUS_PENDING}'
              AND ready_state = '{READY_STATE_READY}'
              AND lane = ?3
              AND (
                  SELECT COUNT(*) FROM step_projection
                  WHERE status = '{STATUS_RUNNING}'
                    AND lane = ?3
              ) < ?4
            ORDER BY created_at ASC, step_sequence ASC
            LIMIT 1
        )
        {RETURNING_COLS}"
    );
    Statement::from_sql_and_values(
        DbBackend::Sqlite,
        sql,
        vec![
            Value::from(worker_id.to_owned()),
            Value::from(now),
            Value::from(lane),
            Value::from(max_concurrent),
        ],
    )
}

fn sqlite_claim_global(worker_id: &str, max_concurrent: i64) -> Statement {
    let now = Utc::now();
    let sql = format!(
        "UPDATE step_projection
        SET status = '{STATUS_RUNNING}',
            worker_id = ?1,
            updated_at = ?2,
            last_heartbeat_at = ?2
        WHERE step_id = (
            SELECT step_id FROM step_projection
            WHERE status = '{STATUS_PENDING}'
              AND ready_state = '{READY_STATE_READY}'
              AND (
                  SELECT COUNT(*) FROM step_projection
                  WHERE status = '{STATUS_RUNNING}'
              ) < ?3
            ORDER BY created_at ASC, step_sequence ASC
            LIMIT 1
        )
        {RETURNING_COLS}"
    );
    Statement::from_sql_and_values(
        DbBackend::Sqlite,
        sql,
        vec![
            Value::from(worker_id.to_owned()),
            Value::from(now),
            Value::from(max_concurrent),
        ],
    )
}

#[cfg(test)]
mod tests {
    use sea_orm::MockDatabase;
    use tanren_domain::Lane;

    use super::*;

    #[test]
    fn postgres_claim_lane_contains_required_clauses() {
        let stmt = postgres_claim_statement("worker-1", Some("impl".to_owned()), 5);
        let sql = stmt.sql;
        assert!(sql.contains("FOR UPDATE SKIP LOCKED"));
        assert!(sql.contains("pending"));
        assert!(sql.contains("ready_state = 'ready'"));
        assert!(sql.contains("last_heartbeat_at"));
        assert!(sql.contains("RETURNING"));
        assert!(sql.contains("lane = $2"), "lane-scoped must use direct eq");
        assert!(
            !sql.contains("IS NULL"),
            "lane-scoped must not use IS NULL pattern"
        );
    }

    #[test]
    fn postgres_claim_global_omits_lane_filter() {
        let stmt = postgres_claim_statement("worker-1", None, 5);
        let sql = stmt.sql;
        assert!(sql.contains("FOR UPDATE SKIP LOCKED"));
        assert!(!sql.contains("lane ="), "global must not filter by lane");
    }

    #[test]
    fn sqlite_claim_lane_has_no_for_update() {
        let stmt = sqlite_claim_statement("worker-1", Some("impl".to_owned()), 5);
        let sql = stmt.sql;
        assert!(!sql.contains("FOR UPDATE"));
        assert!(sql.contains("pending"));
        assert!(sql.contains("last_heartbeat_at"));
        assert!(sql.contains("RETURNING"));
        assert!(sql.contains("lane = ?3"), "lane-scoped must use direct eq");
        assert!(
            !sql.contains("IS NULL"),
            "lane-scoped must not use IS NULL pattern"
        );
    }

    #[test]
    fn sqlite_claim_global_omits_lane_filter() {
        let stmt = sqlite_claim_statement("worker-1", None, 5);
        let sql = stmt.sql;
        assert!(!sql.contains("lane ="), "global must not filter by lane");
    }

    #[test]
    fn postgres_claim_lane_carries_three_parameters() {
        let stmt = postgres_claim_statement("worker-1", Some("impl".to_owned()), 3);
        assert_eq!(stmt.values.as_ref().map_or(0, |v| v.0.len()), 3);
    }

    #[test]
    fn postgres_claim_global_carries_two_parameters() {
        let stmt = postgres_claim_statement("worker-1", None, 3);
        assert_eq!(stmt.values.as_ref().map_or(0, |v| v.0.len()), 2);
    }

    #[test]
    fn sqlite_claim_lane_carries_four_parameters() {
        let stmt = sqlite_claim_statement("worker-1", Some("impl".to_owned()), 3);
        assert_eq!(stmt.values.as_ref().map_or(0, |v| v.0.len()), 4);
    }

    #[test]
    fn sqlite_claim_global_carries_three_parameters() {
        let stmt = sqlite_claim_statement("worker-1", None, 3);
        assert_eq!(stmt.values.as_ref().map_or(0, |v| v.0.len()), 3);
    }

    /// Reject `MySQL` at the dispatcher because we don't ship a
    /// `MySQL` dequeue path.
    #[tokio::test]
    async fn dequeue_impl_rejects_mysql_backend() {
        let conn = MockDatabase::new(DbBackend::MySql).into_connection();
        let params = DequeueParams {
            worker_id: "w".to_owned(),
            lane: Some(Lane::Impl),
            max_concurrent: 1,
        };
        let err = dequeue_impl(&conn, params)
            .await
            .expect_err("mysql must be rejected");
        let StoreError::Conversion { context, reason } = err else {
            unreachable!("expected Conversion variant");
        };
        assert_eq!(context, "job_queue_dequeue::dequeue_impl");
        assert!(reason.contains("MySQL"));
    }

    #[tokio::test]
    async fn dequeue_impl_rejects_max_concurrent_overflow() {
        let conn = MockDatabase::new(DbBackend::Sqlite).into_connection();
        let params = DequeueParams {
            worker_id: "w".to_owned(),
            lane: None,
            max_concurrent: u64::MAX,
        };
        let err = dequeue_impl(&conn, params)
            .await
            .expect_err("overflow must surface");
        assert!(
            matches!(err, StoreError::Conversion { .. }),
            "expected Conversion, got {err:?}"
        );
    }
}
