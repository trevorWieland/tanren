//! Schema migrations.
//!
//! The migration framework is backend-agnostic — each migration is a
//! `MigrationTrait` that manipulates the schema through `SeaORM`'s
//! `SchemaManager`. `SeaORM` tracks applied migrations in the
//! `seaql_migrations` table, which makes `Migrator::up` idempotent:
//! running it twice in a row is a no-op on the second call.

use sea_orm_migration::{MigrationTrait, MigratorTrait, async_trait};

mod m_0001_init;
mod m_0002_integrity;
mod m_0003_dequeue_indexes;
mod m_0004_dispatch_cursor_indexes;
mod m_0005_cancel_dispatch_indexes;
mod m_0006_dispatch_read_scope;
mod m_0007_dispatch_scope_tuple_index;
mod m_0008_dispatch_scope_common_tuple_indexes;
mod m_0009_actor_token_replay;
mod m_0010_projection_enum_constraints;
mod m_0011_dispatch_projection_org_id_not_null;
mod m_0012_methodology_audit_pipeline;
mod m_0013_methodology_read_indexes;
mod m_0014_methodology_idempotency_hash_algo;
mod m_0015_methodology_task_status_projection;
mod m_0016_methodology_task_projection_snapshot;
mod m_0017_methodology_spec_lookup_projection;
mod m_0018_methodology_phase_event_outbox_indexes;
mod m_0019_methodology_idempotency_reservation_lease;

/// Master migrator for the store. Run against a live
/// [`sea_orm::DatabaseConnection`] by
/// [`Store::run_migrations`](crate::Store::run_migrations).
#[derive(Debug)]
pub(crate) struct Migrator;

impl Migrator {
    /// Name of the latest expected schema migration.
    pub(crate) const LATEST_MIGRATION_NAME: &'static str =
        "m_0019_methodology_idempotency_reservation_lease";
}

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m_0001_init::Migration),
            Box::new(m_0002_integrity::Migration),
            Box::new(m_0003_dequeue_indexes::Migration),
            Box::new(m_0004_dispatch_cursor_indexes::Migration),
            Box::new(m_0005_cancel_dispatch_indexes::Migration),
            Box::new(m_0006_dispatch_read_scope::Migration),
            Box::new(m_0007_dispatch_scope_tuple_index::Migration),
            Box::new(m_0008_dispatch_scope_common_tuple_indexes::Migration),
            Box::new(m_0009_actor_token_replay::Migration),
            Box::new(m_0010_projection_enum_constraints::Migration),
            Box::new(m_0011_dispatch_projection_org_id_not_null::Migration),
            Box::new(m_0012_methodology_audit_pipeline::Migration),
            Box::new(m_0013_methodology_read_indexes::Migration),
            Box::new(m_0014_methodology_idempotency_hash_algo::Migration),
            Box::new(m_0015_methodology_task_status_projection::Migration),
            Box::new(m_0016_methodology_task_projection_snapshot::Migration),
            Box::new(m_0017_methodology_spec_lookup_projection::Migration),
            Box::new(m_0018_methodology_phase_event_outbox_indexes::Migration),
            Box::new(m_0019_methodology_idempotency_reservation_lease::Migration),
        ]
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    use super::*;

    /// Round-trip `Migrator::up` and `Migrator::down` against a
    /// fresh in-memory `SQLite` database. This is the only place
    /// the down path is exercised — every other test runs on an
    /// already-migrated database and never tears the schema back
    /// down.
    #[tokio::test]
    async fn up_then_down_is_clean() {
        let conn = Database::connect("sqlite::memory:").await.expect("connect");
        Migrator::up(&conn, None).await.expect("up");
        Migrator::down(&conn, None).await.expect("down");
        // And back up again — the schema should be rebuildable after
        // a full down.
        Migrator::up(&conn, None).await.expect("up after down");
    }

    #[tokio::test]
    async fn up_is_idempotent() {
        let conn = Database::connect("sqlite::memory:").await.expect("connect");
        Migrator::up(&conn, None).await.expect("first up");
        Migrator::up(&conn, None).await.expect("second up");
    }

    #[tokio::test]
    async fn m0006_resume_succeeds_after_partial_column_apply() {
        let conn = Database::connect("sqlite::memory:").await.expect("connect");
        Migrator::up(&conn, Some(5))
            .await
            .expect("up through m_0005");
        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            "ALTER TABLE dispatch_projection ADD COLUMN org_id TEXT NULL",
        ))
        .await
        .expect("partial org_id column");

        Migrator::up(&conn, None).await.expect("resume to latest");
        let rows = conn
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT name FROM pragma_table_info('dispatch_projection') WHERE name IN ('org_id','scope_project_id','scope_team_id','scope_api_key_id')",
            ))
            .await
            .expect("inspect scope columns");
        assert_eq!(
            rows.len(),
            4,
            "all m_0006 scope columns must exist after resumable rerun"
        );
    }

    /// After `m_0011` the `SQLite` triggers must refuse any insert with a
    /// `NULL` `org_id`. This is the compile-time-equivalent regression
    /// guard for Finding 1 of the lane-0.4 audit.
    #[tokio::test]
    async fn m0011_blocks_null_org_id_insert() {
        let conn = Database::connect("sqlite::memory:").await.expect("connect");
        Migrator::up(&conn, None).await.expect("migrate to latest");
        let result = conn
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                INSERT_ROW_WITH_NULL_ORG_ID,
            ))
            .await;
        let err = result.expect_err("inserting null org_id must fail");
        assert!(
            err.to_string().contains("org_id must be non-null"),
            "unexpected trigger error: {err}"
        );
    }

    /// If `org_id` is populated, the trigger must not fire and inserts
    /// proceed normally.
    #[tokio::test]
    async fn m0011_allows_populated_org_id_insert() {
        let conn = Database::connect("sqlite::memory:").await.expect("connect");
        Migrator::up(&conn, None).await.expect("migrate to latest");
        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            INSERT_ROW_WITH_POPULATED_ORG_ID,
        ))
        .await
        .expect("populated org_id insert should succeed");
    }

    /// Pre-`m_0006` rows that contain `actor.org_id` in their JSON but
    /// have `NULL` in the denormalized column must be backfilled by
    /// `m_0011`, not quarantined.
    #[tokio::test]
    async fn m0011_backfills_legacy_row_from_actor_json() {
        let conn = Database::connect("sqlite::memory:").await.expect("connect");
        Migrator::up(&conn, Some(5))
            .await
            .expect("up through m_0005");
        // Seed a dispatch row before m_0006 added the scope columns.
        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            LEGACY_SEED_BEFORE_M0006,
        ))
        .await
        .expect("seed legacy row");

        Migrator::up(&conn, None).await.expect("resume to latest");

        let rows = conn
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT org_id FROM dispatch_projection \
                 WHERE dispatch_id = '11111111-1111-1111-1111-111111111111'",
            ))
            .await
            .expect("select");
        assert_eq!(rows.len(), 1, "row should survive m_0011 backfill");
        let org_id: String = rows[0].try_get("", "org_id").expect("org_id column");
        assert_eq!(org_id, "22222222-2222-2222-2222-222222222222");
    }

    /// Rows whose `actor` JSON lacks `org_id` entirely are quarantined
    /// (row + step rows purged) so the `NOT NULL` enforcement stage
    /// does not reject them.
    #[tokio::test]
    async fn m0011_quarantines_unbackfillable_legacy_row() {
        let conn = Database::connect("sqlite::memory:").await.expect("connect");
        Migrator::up(&conn, Some(5))
            .await
            .expect("up through m_0005");
        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            LEGACY_SEED_MISSING_ORG_ID,
        ))
        .await
        .expect("seed legacy row");

        Migrator::up(&conn, None).await.expect("resume to latest");

        let dispatch_rows = conn
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT dispatch_id FROM dispatch_projection \
                 WHERE dispatch_id = '33333333-3333-3333-3333-333333333333'",
            ))
            .await
            .expect("select");
        assert!(
            dispatch_rows.is_empty(),
            "unbackfillable legacy row must be quarantined"
        );
    }

    /// Running the `down` of `m_0011` must restore the pre-migration
    /// behavior (NULL inserts accepted again). Re-running `up` must
    /// reinstall the enforcement.
    #[tokio::test]
    async fn m0011_down_and_up_round_trip_restores_enforcement() {
        let conn = Database::connect("sqlite::memory:").await.expect("connect");
        Migrator::up(&conn, None).await.expect("up");
        // m_0012..m_0019 now sit above m_0011; roll all nine
        // back so this test exercises m_0011's `down` behavior.
        Migrator::down(&conn, Some(9))
            .await
            .expect("down nine steps");

        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            INSERT_ROW_WITH_NULL_ORG_ID,
        ))
        .await
        .expect("null insert accepted after down");

        // Re-run up; the insert above is still present, so the trigger
        // needs only block future NULL inserts.
        Migrator::up(&conn, None).await.expect("up after down");
        let err = conn
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                INSERT_ROW_WITH_SECOND_NULL_ORG_ID,
            ))
            .await
            .expect_err("post-up null insert must be rejected");
        assert!(
            err.to_string().contains("org_id must be non-null"),
            "unexpected trigger error: {err}"
        );
    }

    // ---------------------------------------------------------------------
    // Test fixtures — kept as crate-local constants so individual tests
    // stay under the workspace function-length lint.
    // ---------------------------------------------------------------------

    const INSERT_ROW_WITH_NULL_ORG_ID: &str = "\
INSERT INTO dispatch_projection (\
  dispatch_id, mode, status, outcome, lane, dispatch, actor, graph_revision, \
  user_id, org_id, scope_project_id, scope_team_id, scope_api_key_id, project, \
  created_at, updated_at) \
VALUES (\
  '44444444-4444-4444-4444-444444444444', 'manual', 'pending', NULL, 'impl', \
  '{}', '{}', 0, '55555555-5555-5555-5555-555555555555', NULL, NULL, NULL, \
  NULL, 'p', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')";

    const INSERT_ROW_WITH_SECOND_NULL_ORG_ID: &str = "\
INSERT INTO dispatch_projection (\
  dispatch_id, mode, status, outcome, lane, dispatch, actor, graph_revision, \
  user_id, org_id, scope_project_id, scope_team_id, scope_api_key_id, project, \
  created_at, updated_at) \
VALUES (\
  '66666666-6666-6666-6666-666666666666', 'manual', 'pending', NULL, 'impl', \
  '{}', '{}', 0, '77777777-7777-7777-7777-777777777777', NULL, NULL, NULL, \
  NULL, 'p', '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')";

    const INSERT_ROW_WITH_POPULATED_ORG_ID: &str = "\
INSERT INTO dispatch_projection (\
  dispatch_id, mode, status, outcome, lane, dispatch, actor, graph_revision, \
  user_id, org_id, scope_project_id, scope_team_id, scope_api_key_id, project, \
  created_at, updated_at) \
VALUES (\
  '88888888-8888-8888-8888-888888888888', 'manual', 'pending', NULL, 'impl', \
  '{}', '{}', 0, '99999999-9999-9999-9999-999999999999', \
  'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa', NULL, NULL, NULL, 'p', \
  '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')";

    const LEGACY_SEED_BEFORE_M0006: &str = "\
INSERT INTO dispatch_projection (\
  dispatch_id, mode, status, outcome, lane, dispatch, actor, graph_revision, \
  user_id, project, created_at, updated_at) \
VALUES (\
  '11111111-1111-1111-1111-111111111111', 'manual', 'pending', NULL, 'impl', \
  '{}', '{\"org_id\":\"22222222-2222-2222-2222-222222222222\",\"user_id\":\"bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb\"}', \
  0, 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb', 'p', \
  '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')";

    const LEGACY_SEED_MISSING_ORG_ID: &str = "\
INSERT INTO dispatch_projection (\
  dispatch_id, mode, status, outcome, lane, dispatch, actor, graph_revision, \
  user_id, project, created_at, updated_at) \
VALUES (\
  '33333333-3333-3333-3333-333333333333', 'manual', 'pending', NULL, 'impl', \
  '{}', '{\"user_id\":\"cccccccc-cccc-cccc-cccc-cccccccccccc\"}', 0, \
  'cccccccc-cccc-cccc-cccc-cccccccccccc', 'p', \
  '2025-01-01T00:00:00Z', '2025-01-01T00:00:00Z')";
}
