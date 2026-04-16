//! `dispatch_projection.org_id` hardening: backfill, quarantine, and
//! enforce `NOT NULL` end-to-end.
//!
//! Background. `m_0006_dispatch_read_scope` added `org_id` as a
//! nullable column and backfilled it from the serialized `actor`
//! JSON. Since then, every write path sets `org_id = Some(...)`
//! because the domain `ActorContext` always carries `org_id`. The
//! column has nonetheless remained nullable at the database and
//! entity level, which meant the cancel-auth lookup
//! ([`crate::StateStore::get_dispatch_actor_context_for_cancel_auth`])
//! had to treat `NULL` as a runtime [`crate::StoreError::Conversion`].
//!
//! This migration closes that integrity gap.
//!
//! Phases (all idempotent, safe to re-run):
//!
//! 1. **Re-backfill** any rows where `org_id IS NULL` by re-extracting
//!    `actor.org_id` from the JSON column. Mirrors the `m_0006` logic
//!    so that a partially-seeded database converges on the same state
//!    reachable by running `m_0006` first.
//! 2. **Quarantine** any row whose `org_id` is still `NULL` after the
//!    backfill. Such rows cannot have been produced by the current
//!    converter and are already runtime-rejected by the cancel-auth
//!    lookup; they would also be unreadable through any scoped query
//!    once the column becomes `NOT NULL`. We delete the dispatch row
//!    and its steps and log one warning per quarantined `dispatch_id`.
//! 3. **Enforce `NOT NULL`**. On Postgres this is a single
//!    `ALTER COLUMN ... SET NOT NULL`, wrapped in an `IF NOT EXISTS`
//!    guard so re-running is a no-op. On `SQLite` the column cannot
//!    be altered in place without a table rebuild, so we install the
//!    same `BEFORE INSERT`/`BEFORE UPDATE` trigger pair pattern
//!    already used by `m_0010_projection_enum_constraints`.

use sea_orm_migration::prelude::*;
use tracing::warn;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0011_dispatch_projection_org_id_not_null"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Box::pin(rebackfill_org_id(manager)).await?;
        Box::pin(quarantine_unbackfillable_rows(manager)).await?;
        Box::pin(enforce_not_null(manager)).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        Box::pin(drop_not_null_enforcement(manager)).await
    }
}

fn is_postgres(manager: &SchemaManager<'_>) -> bool {
    matches!(manager.get_database_backend(), sea_orm::DbBackend::Postgres)
}

async fn rebackfill_org_id(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let sql = if is_postgres(manager) {
        "UPDATE dispatch_projection \
         SET org_id = NULLIF(actor ->> 'org_id', '')::uuid \
         WHERE org_id IS NULL"
    } else {
        "UPDATE dispatch_projection \
         SET org_id = json_extract(actor, '$.org_id') \
         WHERE org_id IS NULL"
    };
    manager.get_connection().execute_unprepared(sql).await?;
    Ok(())
}

async fn quarantine_unbackfillable_rows(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let conn = manager.get_connection();
    let backend = conn.get_database_backend();

    // Read as TEXT to work with both Postgres (uuid as native) and
    // SQLite (uuid stored as hyphenated TEXT). We only need the
    // identifier for logging; the subsequent DELETE uses its own
    // predicate so we do not need to parse a typed UUID here.
    let select_sql = if matches!(backend, sea_orm::DbBackend::Postgres) {
        "SELECT dispatch_id::text AS dispatch_id FROM dispatch_projection WHERE org_id IS NULL"
    } else {
        "SELECT dispatch_id FROM dispatch_projection WHERE org_id IS NULL"
    };
    let select_stmt = sea_orm::Statement::from_string(backend, select_sql.to_owned());
    let rows = conn.query_all(select_stmt).await?;

    if rows.is_empty() {
        return Ok(());
    }

    for row in &rows {
        let id: String = row.try_get("", "dispatch_id")?;
        warn!(
            target: "tanren_store::migration",
            dispatch_id = %id,
            migration = "m_0011_dispatch_projection_org_id_not_null",
            "legacy NULL org_id dispatch row quarantined"
        );
    }

    // Delete step rows first so no orphaned references survive the
    // dispatch row deletion (no FK cascade is declared in m_0001).
    conn.execute_unprepared(
        "DELETE FROM step_projection \
         WHERE dispatch_id IN (SELECT dispatch_id FROM dispatch_projection WHERE org_id IS NULL)",
    )
    .await?;
    conn.execute_unprepared("DELETE FROM dispatch_projection WHERE org_id IS NULL")
        .await?;
    Ok(())
}

async fn enforce_not_null(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if is_postgres(manager) {
        add_postgres_not_null(manager).await
    } else {
        add_sqlite_not_null_triggers(manager).await
    }
}

async fn drop_not_null_enforcement(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if is_postgres(manager) {
        drop_postgres_not_null(manager).await
    } else {
        drop_sqlite_not_null_triggers(manager).await
    }
}

async fn add_postgres_not_null(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r"
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_attribute
        WHERE attrelid = 'dispatch_projection'::regclass
          AND attname  = 'org_id'
          AND attnotnull
    ) THEN
        ALTER TABLE dispatch_projection ALTER COLUMN org_id SET NOT NULL;
    END IF;
END $$;
",
        )
        .await?;
    Ok(())
}

async fn drop_postgres_not_null(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared("ALTER TABLE dispatch_projection ALTER COLUMN org_id DROP NOT NULL;")
        .await?;
    Ok(())
}

async fn add_sqlite_not_null_triggers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r"
CREATE TRIGGER IF NOT EXISTS trg_dispatch_projection_org_id_not_null_insert
BEFORE INSERT ON dispatch_projection
FOR EACH ROW
BEGIN
    SELECT CASE WHEN NEW.org_id IS NULL
        THEN RAISE(ABORT, 'dispatch_projection.org_id must be non-null') END;
END;

CREATE TRIGGER IF NOT EXISTS trg_dispatch_projection_org_id_not_null_update
BEFORE UPDATE ON dispatch_projection
FOR EACH ROW
BEGIN
    SELECT CASE WHEN NEW.org_id IS NULL
        THEN RAISE(ABORT, 'dispatch_projection.org_id must be non-null') END;
END;
",
        )
        .await?;
    Ok(())
}

async fn drop_sqlite_not_null_triggers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r"
DROP TRIGGER IF EXISTS trg_dispatch_projection_org_id_not_null_update;
DROP TRIGGER IF EXISTS trg_dispatch_projection_org_id_not_null_insert;
",
        )
        .await?;
    Ok(())
}
