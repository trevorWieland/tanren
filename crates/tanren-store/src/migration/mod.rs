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
