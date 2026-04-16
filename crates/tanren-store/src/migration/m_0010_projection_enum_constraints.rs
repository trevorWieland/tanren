use sea_orm_migration::prelude::*;

#[derive(Debug)]
pub(crate) struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &'static str {
        "m_0010_projection_enum_constraints"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if is_postgres(manager) {
            add_postgres_constraints(manager).await
        } else {
            add_sqlite_guards(manager).await
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if is_postgres(manager) {
            drop_postgres_constraints(manager).await
        } else {
            drop_sqlite_guards(manager).await
        }
    }
}

fn is_postgres(manager: &SchemaManager<'_>) -> bool {
    matches!(manager.get_database_backend(), sea_orm::DbBackend::Postgres)
}

async fn add_postgres_constraints(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r"
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_dispatch_projection_status_enum'
    ) THEN
        ALTER TABLE dispatch_projection
            ADD CONSTRAINT chk_dispatch_projection_status_enum
            CHECK (status IN ('pending','running','completed','failed','cancelled'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_dispatch_projection_lane_enum'
    ) THEN
        ALTER TABLE dispatch_projection
            ADD CONSTRAINT chk_dispatch_projection_lane_enum
            CHECK (lane IN ('impl','audit','gate'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_dispatch_projection_outcome_enum'
    ) THEN
        ALTER TABLE dispatch_projection
            ADD CONSTRAINT chk_dispatch_projection_outcome_enum
            CHECK (outcome IS NULL OR outcome IN ('success','fail','blocked','error','timeout'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_dispatch_projection_status_outcome_consistency'
    ) THEN
        ALTER TABLE dispatch_projection
            ADD CONSTRAINT chk_dispatch_projection_status_outcome_consistency
            CHECK (
                (status IN ('pending','running','cancelled') AND outcome IS NULL)
                OR (status = 'completed' AND outcome = 'success')
                OR (status = 'failed' AND outcome IN ('fail','blocked','error','timeout'))
            );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_step_projection_status_enum'
    ) THEN
        ALTER TABLE step_projection
            ADD CONSTRAINT chk_step_projection_status_enum
            CHECK (status IN ('pending','running','completed','failed','cancelled'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_step_projection_step_type_enum'
    ) THEN
        ALTER TABLE step_projection
            ADD CONSTRAINT chk_step_projection_step_type_enum
            CHECK (step_type IN ('provision','execute','teardown','dry_run'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_step_projection_ready_state_enum'
    ) THEN
        ALTER TABLE step_projection
            ADD CONSTRAINT chk_step_projection_ready_state_enum
            CHECK (ready_state IN ('blocked','ready'));
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'chk_step_projection_lane_enum'
    ) THEN
        ALTER TABLE step_projection
            ADD CONSTRAINT chk_step_projection_lane_enum
            CHECK (lane IS NULL OR lane IN ('impl','audit','gate'));
    END IF;
END $$;
",
        )
        .await?;
    Ok(())
}

async fn drop_postgres_constraints(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r"
ALTER TABLE dispatch_projection
    DROP CONSTRAINT IF EXISTS chk_dispatch_projection_status_outcome_consistency,
    DROP CONSTRAINT IF EXISTS chk_dispatch_projection_outcome_enum,
    DROP CONSTRAINT IF EXISTS chk_dispatch_projection_lane_enum,
    DROP CONSTRAINT IF EXISTS chk_dispatch_projection_status_enum;

ALTER TABLE step_projection
    DROP CONSTRAINT IF EXISTS chk_step_projection_lane_enum,
    DROP CONSTRAINT IF EXISTS chk_step_projection_ready_state_enum,
    DROP CONSTRAINT IF EXISTS chk_step_projection_step_type_enum,
    DROP CONSTRAINT IF EXISTS chk_step_projection_status_enum;
",
        )
        .await?;
    Ok(())
}

async fn add_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r"
CREATE TRIGGER IF NOT EXISTS trg_dispatch_projection_constraints_insert
BEFORE INSERT ON dispatch_projection
FOR EACH ROW
BEGIN
    SELECT CASE WHEN NEW.status NOT IN ('pending','running','completed','failed','cancelled')
        THEN RAISE(ABORT, 'dispatch_projection.status out of enum') END;
    SELECT CASE WHEN NEW.lane NOT IN ('impl','audit','gate')
        THEN RAISE(ABORT, 'dispatch_projection.lane out of enum') END;
    SELECT CASE WHEN NEW.outcome IS NOT NULL
        AND NEW.outcome NOT IN ('success','fail','blocked','error','timeout')
        THEN RAISE(ABORT, 'dispatch_projection.outcome out of enum') END;
    SELECT CASE WHEN NOT (
        (NEW.status IN ('pending','running','cancelled') AND NEW.outcome IS NULL)
        OR (NEW.status = 'completed' AND NEW.outcome = 'success')
        OR (NEW.status = 'failed' AND NEW.outcome IN ('fail','blocked','error','timeout'))
    ) THEN RAISE(ABORT, 'dispatch_projection status/outcome mismatch') END;
END;

CREATE TRIGGER IF NOT EXISTS trg_dispatch_projection_constraints_update
BEFORE UPDATE ON dispatch_projection
FOR EACH ROW
BEGIN
    SELECT CASE WHEN NEW.status NOT IN ('pending','running','completed','failed','cancelled')
        THEN RAISE(ABORT, 'dispatch_projection.status out of enum') END;
    SELECT CASE WHEN NEW.lane NOT IN ('impl','audit','gate')
        THEN RAISE(ABORT, 'dispatch_projection.lane out of enum') END;
    SELECT CASE WHEN NEW.outcome IS NOT NULL
        AND NEW.outcome NOT IN ('success','fail','blocked','error','timeout')
        THEN RAISE(ABORT, 'dispatch_projection.outcome out of enum') END;
    SELECT CASE WHEN NOT (
        (NEW.status IN ('pending','running','cancelled') AND NEW.outcome IS NULL)
        OR (NEW.status = 'completed' AND NEW.outcome = 'success')
        OR (NEW.status = 'failed' AND NEW.outcome IN ('fail','blocked','error','timeout'))
    ) THEN RAISE(ABORT, 'dispatch_projection status/outcome mismatch') END;
END;

CREATE TRIGGER IF NOT EXISTS trg_step_projection_constraints_insert
BEFORE INSERT ON step_projection
FOR EACH ROW
BEGIN
    SELECT CASE WHEN NEW.status NOT IN ('pending','running','completed','failed','cancelled')
        THEN RAISE(ABORT, 'step_projection.status out of enum') END;
    SELECT CASE WHEN NEW.step_type NOT IN ('provision','execute','teardown','dry_run')
        THEN RAISE(ABORT, 'step_projection.step_type out of enum') END;
    SELECT CASE WHEN NEW.ready_state NOT IN ('blocked','ready')
        THEN RAISE(ABORT, 'step_projection.ready_state out of enum') END;
    SELECT CASE WHEN NEW.lane IS NOT NULL AND NEW.lane NOT IN ('impl','audit','gate')
        THEN RAISE(ABORT, 'step_projection.lane out of enum') END;
END;

CREATE TRIGGER IF NOT EXISTS trg_step_projection_constraints_update
BEFORE UPDATE ON step_projection
FOR EACH ROW
BEGIN
    SELECT CASE WHEN NEW.status NOT IN ('pending','running','completed','failed','cancelled')
        THEN RAISE(ABORT, 'step_projection.status out of enum') END;
    SELECT CASE WHEN NEW.step_type NOT IN ('provision','execute','teardown','dry_run')
        THEN RAISE(ABORT, 'step_projection.step_type out of enum') END;
    SELECT CASE WHEN NEW.ready_state NOT IN ('blocked','ready')
        THEN RAISE(ABORT, 'step_projection.ready_state out of enum') END;
    SELECT CASE WHEN NEW.lane IS NOT NULL AND NEW.lane NOT IN ('impl','audit','gate')
        THEN RAISE(ABORT, 'step_projection.lane out of enum') END;
END;
",
        )
        .await?;
    Ok(())
}

async fn drop_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r"
DROP TRIGGER IF EXISTS trg_step_projection_constraints_update;
DROP TRIGGER IF EXISTS trg_step_projection_constraints_insert;
DROP TRIGGER IF EXISTS trg_dispatch_projection_constraints_update;
DROP TRIGGER IF EXISTS trg_dispatch_projection_constraints_insert;
",
        )
        .await?;
    Ok(())
}
