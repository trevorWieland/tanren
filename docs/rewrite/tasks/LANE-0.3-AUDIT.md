# Lane 0.3 — Store Core — Audit Brief

## Role

You are auditing the `tanren-store` crate implementation. This crate owns
**all database access in the workspace** — SQL, migrations, transactions,
and race-safety. A bug here corrupts data, double-executes work, or leaks
secrets. You are the last line of defense before Lane 0.4 consumes this
store through the real trait implementations.

You must not rubber-stamp. The store layer has subtle correctness
requirements (atomic dequeue, transactional ack_and_enqueue, migration
idempotency) that typical code review misses. This audit exists to
catch those specifically.

## Required Reading

Before auditing, read these in order:

1. `docs/rewrite/tasks/LANE-0.3-STORE.md` — the implementation spec
2. `docs/rewrite/tasks/LANE-0.3-BRIEF.md` — the agent handoff with
   "done when" checklist
3. `docs/rewrite/tasks/ADDON-SEAORM.md` — why SeaORM, what it does and
   doesn't abstract away
4. `docs/rewrite/DESIGN_PRINCIPLES.md` — decision rules
5. `docs/rewrite/CRATE_GUIDE.md` — linking rules
6. `CLAUDE.md` — workspace quality conventions
7. Skim `crates/tanren-domain/src/lib.rs` exports — the store converts
   to/from these types

## Audit Dimensions

### 1. Spec Fidelity — Intent, Not Letter

The spec tells the agent *what* to build. You are checking whether the
implementation honors the deeper intent from the planning docs:

- **Event-sourced durability, projection-driven read performance.**
  Events are the source of truth; projections are derived. Does the
  implementation treat events as canonical? Are projections only
  updated through event append transactions, never independently?
- **No scan-heavy operational paths.** Every operational query should
  use an index. Run `EXPLAIN QUERY PLAN` (SQLite) or `EXPLAIN ANALYZE`
  (Postgres) on at least: `get_dispatch`, `get_steps_for_dispatch`,
  `count_running_steps`, `query_dispatches` with a status filter.
  Full table scans on any of these are a blocker.
- **Single backend abstraction, documented escape hatches.** Raw SQL is
  permitted only where SeaORM's entity API cannot express the query
  (the `dequeue` claim path). Anywhere else raw SQL appears, it should
  be justified in a comment. Ad-hoc raw SQL for "convenience" is a
  regression.
- **No domain logic in the store.** The store persists and queries. It
  does not decide. Things like "should this dispatch be cancelled" or
  "is this guard violated" belong in the orchestrator (Lane 0.4) and
  use domain functions. If the store calls `check_execute_guards` or
  similar, that is a layering violation.

### 2. Correctness

- **State machine fidelity.** Reads and writes must preserve the
  lifecycle enums exactly as domain defines them. String coercion
  (e.g., `status.to_string()` on write, parse on read) is a common
  bug vector. Verify roundtrip correctness by reading back every
  write and comparing to the input. Snapshot tests at the SQL layer
  are helpful here.
- **Event payload round-trip.** Events stored as JSON must come back
  equal to what went in. Check that the converter uses
  `serde_json::to_value` / `from_value` (not `to_string` / `from_str`)
  so the SeaORM entity path matches what the Lane 0.2 audit certified.
- **Sequence numbering.** If the store assigns `step_sequence`
  values, they must be monotonic per dispatch and gap-free under normal
  operation. Verify with a test that creates a dispatch, enqueues
  several steps, and reads back the sequence.
- **Projection consistency with events.** After appending a
  `DispatchCreated` event, `get_dispatch` must return the new
  dispatch immediately in the same transaction. After appending a
  `StepCompleted` event via `ack`, `get_step` must reflect the new
  status.

### 3. Race Safety — The Critical Section

This is the most important dimension. Most store bugs hide here.

- **`dequeue` atomicity.** The TOCTOU hazard is: read
  `count(status='running' AND lane=X)`, decide to claim, mark
  `status='running'`. Between read and mark, another worker can claim
  the same slot. Verify the implementation uses a single transaction
  with proper isolation:
  - **Postgres path:** `SELECT ... FOR UPDATE SKIP LOCKED` is the
    correct idiom. The `UPDATE ... WHERE step_id = (subquery)` pattern
    with `FOR UPDATE SKIP LOCKED` inside the subquery is canonical.
    Any other pattern (separate SELECT then UPDATE, no row-level lock,
    `SERIALIZABLE` without `SKIP LOCKED`) is suspect.
  - **SQLite path:** SQLite has no row-level locks. The correct
    approach is `BEGIN IMMEDIATE` + `busy_timeout` to force
    single-writer serialization on the database, plus the count check
    and update in one transaction. If the code uses `BEGIN DEFERRED`
    or no explicit `BEGIN IMMEDIATE`, flag it.
- **Concurrency test actually tests concurrency.** Check that a
  concurrency test exists and that it meaningfully exercises the race.
  A test that spawns two tasks serially does nothing; a test that
  spawns N tasks with `tokio::spawn` and then awaits all of them
  against Postgres is correct. Verify the test:
  - Runs against Postgres (SQLite can't exercise row-level locking)
  - Spawns ≥ 10 concurrent claim attempts
  - Asserts that the number of successful claims equals the number
    of available slots (not more, not less)
  - Asserts that no step is claimed by two workers
- **`ack_and_enqueue` atomicity.** The implementation must do:
  1. Update current step `status='completed'`, store result
  2. Insert next step row
  3. Append `StepCompleted` event
  4. Append `StepEnqueued` event
  — all in **one** `db.transaction::<_, _, StoreError>(...)` closure.
  Any separation is a bug. Verify by reading the code path and
  confirming a single transaction boundary. Simulate a failure mid-op
  in a test (e.g., inject an error after step 2) and confirm no
  partial state remains.
- **`cancel_pending_steps` ordering.** When cancelling pending steps,
  teardown steps should be excluded (per domain guard rules). Verify
  the query filter explicitly excludes `step_type='teardown'`.
- **Crash recovery.** `recover_stale_steps(timeout_secs)` must reset
  running steps older than the threshold back to pending without
  clobbering steps a live worker is still processing. Check the
  heartbeat / updated_at logic and verify with a test.

### 4. Transaction Boundaries

- **Every method that writes and reads in the same operation uses a
  transaction.** Common bugs: append event outside the projection-update
  transaction; ack step but insert next step in a separate await.
- **Error paths roll back cleanly.** If any step in a transaction
  returns an error, the entire transaction must roll back. SeaORM's
  `db.transaction::<_, _, StoreError>(|txn| ...)` does this correctly
  if the closure returns `Err`. Verify there are no manual commit
  calls that bypass the closure's automatic rollback on error.
- **No silent commit-on-drop.** Check that no `Transaction` handle is
  held across an `await` without being either committed or dropped in
  an error path. Silent commit-on-drop is a common footgun with some
  async DB clients; SeaORM's pattern avoids this but verify the code
  uses the closure-based API, not the manual `begin()` / `commit()`
  pattern, unless there's a documented reason.

### 5. Migration Lifecycle

- **Idempotency.** Running `run_migrations` twice must be a no-op on
  the second call. Verify the migration framework uses a
  `seaql_migrations` table (SeaORM's default) or equivalent version
  tracking. Verify with a test that applies migrations twice and
  asserts no error and no schema drift.
- **Fresh DB works on both backends.** Create a fresh SQLite `:memory:`
  DB, apply migrations, verify the expected tables exist. Same for
  Postgres via testcontainers.
- **Schema matches entity definitions.** For every entity in
  `entity/`, verify the migration creates a column matching the entity
  type. A common bug: entity declares `Uuid` but migration creates
  `string()`. This compiles but fails at runtime on the first insert.
- **JsonBinary column type.** Verify the `payload` columns use
  `json_binary()` in the migration (not `json()` or `text()`). This is
  the piece that delivers JSONB on Postgres and TEXT on SQLite.

### 6. Performance

- **Indexes match query patterns.** For each query in the code, identify
  the `WHERE` / `ORDER BY` columns and verify a matching index exists
  in the migration. The spec required indexes are listed — verify each
  is present. Extra indexes are fine; missing ones are a blocker.
- **No N+1 queries.** `get_steps_for_dispatch` should be one query, not
  one query per step. Verify by inspection.
- **Batch append.** `append_batch` should use a single transaction or
  a multi-row insert, not append events one at a time in a loop.
  Verify by inspection.
- **Connection pool sizing.** SeaORM's `ConnectOptions` should set
  reasonable `min_connections` / `max_connections` / `idle_timeout`
  defaults. If the code uses SeaORM's defaults, note that in the
  report. If it sets custom values, verify they are justified.

### 7. Security

- **No secret leakage in error messages.** `StoreError` variants must
  not embed raw SQL, connection strings, or query parameters that
  could contain user-provided values. Check each `#[error(...)]`
  format string and each `From<DbErr>` conversion.
- **No secret leakage in logs.** If the store uses `tracing` (it
  should not heavily instrument in Phase 0), verify no span records
  event payloads or dispatch snapshots with `Debug` formatting. The
  Lane 0.2 audit verified `ConfigEnv::Debug` redacts values, but
  the store could still log the full `DispatchSnapshot` which has
  `ConfigKeys` (names only, safe) — verify this has not regressed.
- **Parameterized queries only.** Every raw SQL string must use
  parameter placeholders (`$1`, `$2` for Postgres; `?` for SQLite)
  and pass values through `Statement::from_sql_and_values`. String
  interpolation of user data into SQL is SQL injection, blocker.
- **No stored credentials.** The store should never persist raw
  secret values. Events should only contain names (`required_secrets:
  Vec<String>`) never values. Verify by grepping for likely secret
  field names and confirming they don't appear in any insert path.

### 8. Maintainability

- **No file over 500 lines, no function over 100.** Run
  `just check-lines`. A store implementation tends to grow long — if
  a single file is approaching the limit, it should have been split.
- **Entity definitions are authoritative.** Columns in `entity/events.rs`
  must match columns created in `migration/m_0001_init.rs`. Drift
  here is a runtime landmine. A good pattern: one integration test
  that round-trips every entity through a fresh DB.
- **Converters are symmetric.** For every `From<Model> for DomainType`
  there should be a corresponding `impl From<DomainType> for ActiveModel`
  (or `TryFrom` if fallible). Asymmetric converters are a smell.
- **No inline `#[allow]`.** `just check-suppression` must pass.
- **Doc comments on every public item.** SeaORM entity code is
  boilerplate-heavy but the trait impls and converters should have
  `///` doc comments explaining non-obvious choices.

### 9. Testing Quality

- **Three tiers actually implemented.** `MockDatabase` unit tests,
  SQLite in-memory integration tests, and Postgres testcontainers
  integration tests all exist. Verify by running each tier explicitly:
  - `cargo nextest run -p tanren-store` (default tier)
  - `cargo nextest run -p tanren-store --features postgres-integration`
    (Postgres tier, requires running Postgres)
- **Test coverage is meaningful, not just numbers.** Run
  `cargo llvm-cov nextest -p tanren-store --summary-only`. Target
  is ≥ 85% line coverage. Flag coverage gaps in the trait method
  implementations specifically — a 70% overall with 50% coverage on
  `JobQueue::dequeue` is worse than 80% with 100% on dequeue.
- **Regression tests cover the prior-audit risks.** Verify the
  implementation has explicit tests for:
  - Double-claim prevention under concurrent dequeue
  - `ack_and_enqueue` atomic rollback on mid-op failure
  - Migration double-apply idempotency
  - Event round-trip through `serde_json::Value` (not string)

### 10. Scalability and Extensibility

- **Adding a new event variant.** If a new `DomainEvent` variant is
  added in Lane 0.2 (or later), how much of this store breaks? Ideally
  zero — the event payload is `serde_json::Value` and the store
  doesn't care what's inside. Verify by inspection.
- **Adding a new step type.** Same question for `StepType`. The store
  persists the discriminant string and the full payload JSON —
  adding a variant should require no store changes.
- **Adding a new projection table.** When Phase 3 adds user/apikey
  projections, how invasive is it? The answer should be: add a new
  entity module, add a new migration, add store methods. No existing
  code should need to change. Verify the current module boundaries
  support this cleanly.

## Audit Process

1. Check out the branch under review. Confirm you're on `lane-0.3` or
   its successor (`git branch --show-current`).
2. Run `just ci`. It must pass. Stop and report if it doesn't.
3. Run `cargo nextest run -p tanren-store` (default SQLite tier).
4. Start a Postgres container and run `cargo nextest run -p tanren-store
   --features postgres-integration`. Verify the Postgres tier passes.
5. Run `cargo llvm-cov nextest -p tanren-store --summary-only` and
   record the coverage numbers per file.
6. Walk through each audit dimension above. For each concrete check,
   read the relevant code and either verify it or produce a finding.
7. Pay special attention to Dimension 3 (Race Safety) and Dimension 4
   (Transaction Boundaries) — these are the highest-risk areas.
8. Produce the output report.

## Output Format

Deliver findings in three sections.

### Section 1 — Verification Results

A table of the ten audit dimensions with a one-line verdict and any
blocker/issue/suggestion findings referenced by ID:

```
Dimension                  | Verdict      | Findings
1. Spec Fidelity            | PASS         | —
2. Correctness              | PASS         | S-02
3. Race Safety              | CONCERNS     | B-01, I-03
4. Transaction Boundaries   | PASS         | —
5. Migration Lifecycle      | PASS         | —
6. Performance              | CONCERNS     | I-04
7. Security                 | PASS         | —
8. Maintainability          | PASS         | S-05
9. Testing Quality          | PASS         | —
10. Scalability             | PASS         | —
```

### Section 2 — Findings

Each finding formatted as:

```
### [BLOCKER|ISSUE|SUGGESTION] ID — Short title

**Dimension:** Race Safety / Correctness / etc.
**Location:** crates/tanren-store/src/job_queue.rs:142
**Finding:** Description of what's wrong and why it matters.
**Reproduction:** (for blockers) Concrete steps or test case that
demonstrates the issue.
**Fix:** Concrete fix or direction.
```

Use IDs: `B-NN` for blockers, `I-NN` for issues, `S-NN` for suggestions.

### Section 3 — Recommendation

One of:

- **APPROVE for merge** — no blockers, store is production-ready for
  Phase 0 scope. Any issues are follow-ups, not gates.
- **APPROVE with follow-up** — no blockers, but specific fixes should
  land before Lane 0.4 integration. List them.
- **REJECT** — one or more blockers. List them and describe the fix
  path. Lane 0.3 must re-audit before merge.

Include coverage numbers and the pass/fail status of each test tier as
an appendix to the recommendation.
