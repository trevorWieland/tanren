//! Schema migrations.
//!
//! The migration framework is backend-agnostic — each migration is a
//! `MigrationTrait` that manipulates the schema through `SeaORM`'s
//! `SchemaManager`. `SeaORM` tracks applied migrations in the
//! `seaql_migrations` table, which makes `Migrator::up` idempotent:
//! running it twice in a row is a no-op on the second call.

use sea_orm_migration::{MigrationTrait, MigratorTrait, async_trait};

mod m_0001_init;

/// Master migrator for the store. Run against a live
/// [`sea_orm::DatabaseConnection`] by
/// [`Store::run_migrations`](crate::Store::run_migrations).
#[derive(Debug)]
pub(crate) struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m_0001_init::Migration)]
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;

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
}
