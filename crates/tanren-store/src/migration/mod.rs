//! Migration runner for the Tanren event store.

use sea_orm_migration::MigrationTrait;
use sea_orm_migration::MigratorTrait;

mod m20260501_000001_init;
mod m20260502_000001_accounts;
mod m20260503_000002_account_sessions_expires_at;
mod m20260503_000003_password_phc;
mod m20260506_000004_organizations;

/// Tanren's migration runner. Applied via [`Store::migrate`](crate::Store::migrate).
pub struct Migrator;

impl std::fmt::Debug for Migrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migrator").finish()
    }
}

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260501_000001_init::Migration),
            Box::new(m20260502_000001_accounts::Migration),
            Box::new(m20260503_000002_account_sessions_expires_at::Migration),
            Box::new(m20260503_000003_password_phc::Migration),
            Box::new(m20260506_000004_organizations::Migration),
        ]
    }
}
