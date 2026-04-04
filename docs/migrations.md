# Alembic Migrations

Tanren uses Alembic for database schema migrations. The ORM models in
`packages/tanren-core/src/tanren_core/store/models.py` are the single source
of truth for the database schema.

## Schema Definition

The schema is defined using SQLAlchemy 2.0 `Mapped` classes in `models.py`.
All models inherit from a shared `DeclarativeBase`. The type system adapts
automatically between backends:

| SQLAlchemy type | SQLite | PostgreSQL |
|-----------------|--------|------------|
| `JSON` | TEXT | JSONB |
| `Boolean` | INTEGER | BOOLEAN |
| `BigInteger` | INTEGER | BIGINT |

### Tables

| Table | Purpose |
|-------|---------|
| `events` | Append-only event log (all domain events) |
| `dispatch_projection` | Materialized view of dispatch state |
| `step_projection` | Job queue backing store -- one row per lifecycle step |
| `vm_assignments` | VM lifecycle tracking (assignment and release) |
| `user_projection` | User account projection |
| `api_key_projection` | API key projection with scopes and resource limits |

## Development vs Production

- **Development**: `create_store()` (in `store/factory.py`) calls
  `Base.metadata.create_all`, which creates tables if they don't exist.
  This is convenient for local dev and tests but does not track migrations.
- **Production**: Use Alembic to apply migrations. This provides version
  tracking, rollback capability, and CI-verifiable schema drift detection.

The initial Alembic migration was generated from the ORM models (not from
legacy DDL strings), establishing a clean baseline.

## Creating a Migration

After modifying models in `models.py`, generate a migration:

```bash
cd packages/tanren-core
uv run alembic revision --autogenerate -m "description of change"
```

This compares the current ORM models against the database and generates a
migration script in `packages/tanren-core/alembic/versions/`.

Review the generated migration before committing -- autogenerate does not
detect all changes (e.g., column renames, data migrations).

## Applying Migrations

```bash
cd packages/tanren-core
uv run alembic upgrade head
```

## Configuration

Alembic configuration is in `packages/tanren-core/alembic.ini`:

- **Database URL**: `sqlalchemy.url` (default: `sqlite+aiosqlite:///tanren.db`)
- **Async engine**: `alembic/env.py` uses `async_engine_from_config` with
  `asyncio.run()` for online migrations

The `env.py` imports `Base` from `tanren_core.store.models` to provide
`target_metadata` for autogenerate.

## CI Verification

The `make alembic-check` target verifies that the ORM models and migration
history are in sync:

```bash
make alembic-check
# Runs: cd packages/tanren-core && uv run alembic upgrade head && uv run alembic check
```

This is part of the `arch-check` target, which runs in CI via `make check`.
The two-step process:

1. `alembic upgrade head` -- applies all migrations to a fresh database
2. `alembic check` -- compares ORM models against the migrated schema,
   failing if any drift is detected
