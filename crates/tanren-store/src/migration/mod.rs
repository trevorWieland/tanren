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

/// Master migrator for the store. Run against a live
/// [`sea_orm::DatabaseConnection`] by
/// [`Store::run_migrations`](crate::Store::run_migrations).
#[derive(Debug)]
pub(crate) struct Migrator;

impl Migrator {
    /// Name of the latest expected schema migration.
    pub(crate) const LATEST_MIGRATION_NAME: &'static str = "m_0007_dispatch_scope_tuple_index";
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
}
