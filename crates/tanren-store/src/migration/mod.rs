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
mod m_0020_methodology_task_finding_projection;

/// Master migrator for the store. Run against a live
/// [`sea_orm::DatabaseConnection`] by
/// [`Store::run_migrations`](crate::Store::run_migrations).
#[derive(Debug)]
pub(crate) struct Migrator;

impl Migrator {
    /// Name of the latest expected schema migration.
    pub(crate) const LATEST_MIGRATION_NAME: &'static str =
        "m_0020_methodology_task_finding_projection";
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
            Box::new(m_0020_methodology_task_finding_projection::Migration),
        ]
    }
}
