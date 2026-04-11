//! Atomic dequeue — the one place raw SQL remains.
//!
//! `dequeue` must perform the "check running-count, claim pending
//! candidate" decision in a single atomic step, otherwise two workers
//! will see the same pending row before either of them flips it to
//! `running`. `SeaORM`'s entity API cannot express this: there's no
//! dialect-neutral way to encode `SELECT ... FOR UPDATE SKIP LOCKED`
//! (`Postgres`) or a guaranteed single-writer `SQLite` `UPDATE` with
//! subquery count-check. This module therefore branches on
//! `DbBackend` and hands each backend the canonical idiom for its
//! locking model:
//!
//! - **`Postgres`** — `UPDATE ... WHERE step_id = (SELECT ... FOR
//!   UPDATE SKIP LOCKED LIMIT 1) RETURNING ...`. Row-level locking
//!   ensures two workers cannot pick the same candidate, and `SKIP
//!   LOCKED` lets workers slide past rows already being considered.
//!
//! - **`SQLite`** — the same single-statement shape, minus the
//!   `FOR UPDATE` clause. `SQLite` holds its implicit write lock
//!   across the whole statement, so the subquery and the update
//!   execute together. The `busy_timeout` URL parameter handles
//!   contention between processes without propagating `SQLITE_BUSY`
//!   up.
//!
//! Both variants embed the `running`-count check inside the WHERE
//! clause so the lane concurrency cap cannot be violated under race.

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, Statement, Value};

use crate::converters::dispatch as dispatch_converters;
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
    let lane_string = params
        .lane
        .map(|l| dispatch_converters::lane_to_string(l).to_owned());

    let backend = conn.get_database_backend();
    let statement = match backend {
        DbBackend::Postgres => postgres_statement(&params.worker_id, lane_string, max_concurrent),
        DbBackend::Sqlite => sqlite_statement(&params.worker_id, lane_string, max_concurrent),
        DbBackend::MySql => {
            return Err(StoreError::Conversion {
                context: "job_queue_dequeue::dequeue_impl",
                reason: "MySQL is not a supported backend".to_owned(),
            });
        }
    };

    // `find().from_raw_sql(...)` lets SeaORM reconstruct a full
    // `step_projection::Model` from the RETURNING row without
    // manual column extraction. This keeps the converter path the
    // same as every other read: go through `model_to_queued_step`.
    let row = step_projection::Entity::find()
        .from_raw_sql(statement)
        .one(conn)
        .await?;

    match row {
        Some(model) => Ok(Some(step_converters::model_to_queued_step(model)?)),
        None => Ok(None),
    }
}

fn postgres_statement(worker_id: &str, lane: Option<String>, max_concurrent: i64) -> Statement {
    let sql = r"
        UPDATE step_projection
        SET status = 'running',
            worker_id = $1,
            updated_at = NOW()
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
                  result, error, retry_count, created_at, updated_at
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

fn sqlite_statement(worker_id: &str, lane: Option<String>, max_concurrent: i64) -> Statement {
    // SQLite's `DATETIME('now')` returns a TIMESTAMP without
    // sub-second precision. We parameter-bind the current time
    // instead, so the column writes are comparable across backends.
    let now = chrono::Utc::now();
    let sql = r"
        UPDATE step_projection
        SET status = 'running',
            worker_id = ?1,
            updated_at = ?2
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
                  result, error, retry_count, created_at, updated_at
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
