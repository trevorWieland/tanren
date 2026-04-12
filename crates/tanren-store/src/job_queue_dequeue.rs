//! Atomic dequeue — the one place raw SQL remains.
//!
//! `dequeue` must perform the "check running-count, claim pending
//! candidate" decision in a single atomic step, otherwise two workers
//! will see the same pending row before either of them flips it to
//! `running`. `SeaORM`'s entity API cannot express this: there's no
//! dialect-neutral way to encode `SELECT ... FOR UPDATE SKIP LOCKED`
//! (`Postgres`) or the serialization required on `SQLite`. This
//! module therefore branches on `DbBackend` and hands each backend
//! the canonical idiom for its locking model.
//!
//! Both branches run inside an explicit transaction and claim at
//! most one candidate row per call.
//!
//! ## `Postgres` path
//!
//! A single global `pg_advisory_xact_lock(42, 0)` is taken at the
//! start of every dequeue transaction, regardless of the requested
//! lane. This serializes all concurrent dequeue operations against
//! the same cluster.
//!
//! A per-lane lock key would be tempting, but `lane=None` counts
//! running rows across *all* lanes while `lane=Impl` counts only
//! impl rows — those are overlapping predicate spaces, so keying
//! by lane lets two callers with different specs both observe a
//! passing count and over-claim. The global lock closes this race
//! at the cost of serializing cross-lane dequeues. The throughput
//! impact is negligible: the critical section is a single UPDATE
//! that completes in microseconds.
//!
//! ## `SQLite` path
//!
//! `SQLite` is serialized at the database level by its single-writer
//! lock. The dequeue is a single `UPDATE ... WHERE step_id =
//! (SELECT ... LIMIT 1) RETURNING ...` statement — there are no
//! pre-write reads in the transaction. `SQLite` acquires the
//! **reserved lock** (equivalent to the write lock) when the first
//! write statement executes, so the entire count+claim decision
//! runs under that lock even under `BEGIN DEFERRED`. This makes the
//! `DEFERRED` vs `IMMEDIATE` distinction a no-op for this specific
//! statement shape: both modes acquire the write lock before the
//! UPDATE's subquery reads the candidate set. The `busy_timeout`
//! set in `connection.rs` handles cross-process contention by
//! retrying inside the driver instead of bubbling `SQLITE_BUSY` up.
//!
//! We use `conn.transaction()` (which issues `BEGIN DEFERRED`) and
//! document this reasoning explicitly so the audit trail is clear.
//! Reference: <https://www.sqlite.org/lang_transaction.html>
//! ("DEFERRED ... the first read operation ... creates a SHARED
//! lock and the first write operation creates a RESERVED lock").

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement, TransactionTrait, Value,
};

use crate::converters::step as step_converters;
use crate::entity::step_projection;
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
    let lane_string = params.lane.map(|l| l.to_string());
    let worker_id = params.worker_id;

    let backend = conn.get_database_backend();
    match backend {
        DbBackend::Postgres => dequeue_postgres(conn, worker_id, lane_string, max_concurrent).await,
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

/// All dequeue operations share key 0. See the module doc for
/// why a per-lane key is unsound.
const ADVISORY_LOCK_KEY: i32 = 0;

async fn dequeue_postgres(
    conn: &DatabaseConnection,
    worker_id: String,
    lane: Option<String>,
    max_concurrent: i64,
) -> StoreResult<Option<QueuedStep>> {
    conn.transaction::<_, Option<QueuedStep>, StoreError>(move |txn| {
        Box::pin(async move {
            // Global serialization lock — see module doc.
            txn.execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT pg_advisory_xact_lock($1, $2)",
                vec![ADVISORY_LOCK_NAMESPACE.into(), ADVISORY_LOCK_KEY.into()],
            ))
            .await?;

            let stmt = postgres_claim_statement(&worker_id, lane, max_concurrent);
            let row = step_projection::Entity::find()
                .from_raw_sql(stmt)
                .one(txn)
                .await?;
            match row {
                Some(model) => Ok(Some(step_converters::model_to_queued_step(model)?)),
                None => Ok(None),
            }
        })
    })
    .await
    .map_err(StoreError::from)
}

/// `SQLite` dequeue inside a `conn.transaction()` closure.
///
/// See the module doc for why `BEGIN DEFERRED` is sufficient for
/// this single-statement shape — the write lock is acquired at
/// the UPDATE, before the subquery reads.
async fn dequeue_sqlite(
    conn: &DatabaseConnection,
    worker_id: String,
    lane: Option<String>,
    max_concurrent: i64,
) -> StoreResult<Option<QueuedStep>> {
    conn.transaction::<_, Option<QueuedStep>, StoreError>(move |txn| {
        Box::pin(async move {
            let stmt = sqlite_claim_statement(&worker_id, lane, max_concurrent);
            let row = step_projection::Entity::find()
                .from_raw_sql(stmt)
                .one(txn)
                .await?;
            match row {
                Some(model) => Ok(Some(step_converters::model_to_queued_step(model)?)),
                None => Ok(None),
            }
        })
    })
    .await
    .map_err(StoreError::from)
}

/// Build the `Postgres` `UPDATE...RETURNING` claim statement.
pub(crate) fn postgres_claim_statement(
    worker_id: &str,
    lane: Option<String>,
    max_concurrent: i64,
) -> Statement {
    let sql = r"
        UPDATE step_projection
        SET status = 'running',
            worker_id = $1,
            updated_at = NOW(),
            last_heartbeat_at = NOW()
        WHERE step_id = (
            SELECT step_id FROM step_projection
            WHERE status = 'pending'
              AND ready_state = 'ready'
              AND ($2::text IS NULL OR lane = $2)
              AND (
                  SELECT COUNT(*) FROM step_projection
                  WHERE status = 'running'
                    AND ($2::text IS NULL OR lane = $2)
              ) < $3
            ORDER BY created_at ASC, step_sequence ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING step_id, dispatch_id, step_type, step_sequence, lane, status,
                  ready_state, depends_on, graph_revision, worker_id, payload,
                  result, error, retry_count, last_heartbeat_at, created_at,
                  updated_at
    ";
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

/// Build the `SQLite` `UPDATE ... RETURNING` claim statement.
pub(crate) fn sqlite_claim_statement(
    worker_id: &str,
    lane: Option<String>,
    max_concurrent: i64,
) -> Statement {
    let now = chrono::Utc::now();
    let sql = r"
        UPDATE step_projection
        SET status = 'running',
            worker_id = ?1,
            updated_at = ?2,
            last_heartbeat_at = ?2
        WHERE step_id = (
            SELECT step_id FROM step_projection
            WHERE status = 'pending'
              AND ready_state = 'ready'
              AND (?3 IS NULL OR lane = ?3)
              AND (
                  SELECT COUNT(*) FROM step_projection
                  WHERE status = 'running'
                    AND (?3 IS NULL OR lane = ?3)
              ) < ?4
            ORDER BY created_at ASC, step_sequence ASC
            LIMIT 1
        )
        RETURNING step_id, dispatch_id, step_type, step_sequence, lane, status,
                  ready_state, depends_on, graph_revision, worker_id, payload,
                  result, error, retry_count, last_heartbeat_at, created_at,
                  updated_at
    ";
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

#[cfg(test)]
mod tests {
    use sea_orm::MockDatabase;
    use tanren_domain::Lane;

    use super::*;

    #[test]
    fn postgres_claim_statement_contains_required_clauses() {
        let stmt = postgres_claim_statement("worker-1", Some("impl".to_owned()), 5);
        let sql = stmt.sql;
        assert!(sql.contains("FOR UPDATE SKIP LOCKED"));
        assert!(sql.contains("pending"));
        assert!(sql.contains("ready_state = 'ready'"));
        assert!(sql.contains("last_heartbeat_at"));
        assert!(sql.contains("RETURNING"));
    }

    #[test]
    fn sqlite_claim_statement_has_no_for_update() {
        let stmt = sqlite_claim_statement("worker-1", Some("impl".to_owned()), 5);
        let sql = stmt.sql;
        assert!(!sql.contains("FOR UPDATE"));
        assert!(sql.contains("pending"));
        assert!(sql.contains("last_heartbeat_at"));
        assert!(sql.contains("RETURNING"));
    }

    #[test]
    fn postgres_claim_statement_carries_three_parameters() {
        let stmt = postgres_claim_statement("worker-1", None, 3);
        assert_eq!(stmt.values.as_ref().map_or(0, |v| v.0.len()), 3);
    }

    #[test]
    fn sqlite_claim_statement_carries_four_parameters() {
        let stmt = sqlite_claim_statement("worker-1", None, 3);
        assert_eq!(stmt.values.as_ref().map_or(0, |v| v.0.len()), 4);
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
