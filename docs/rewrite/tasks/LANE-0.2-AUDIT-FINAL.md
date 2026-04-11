# Lane 0.2 — Domain Model — Final Audit Brief

## Role

You are performing the **final audit** of the `tanren-domain` crate before
it is frozen as the foundation for Lanes 0.3 and 0.4. A prior audit of the
same crate found several issues (detailed below). Your job has three parts:

1. **Verify the prior audit's findings were addressed** (or consciously deferred).
2. **Check for regressions or new issues** introduced since the prior audit.
3. **Certify SeaORM compatibility** — the store layer has shifted from raw
   `sqlx` to SeaORM, and domain types must round-trip cleanly through the
   JSON column types SeaORM emits.

You should not rubber-stamp. You are the last line of defense before the
foundation freezes.

## Required Reading

Before auditing, read these in order:

1. `docs/rewrite/tasks/LANE-0.2-DOMAIN.md` — the implementation spec
2. `docs/rewrite/tasks/LANE-0.2-AUDIT.md` — the original audit brief
   (dimensions and expected scope)
3. `docs/rewrite/DESIGN_PRINCIPLES.md` — the 10 decision rules
4. `docs/rewrite/CRATE_GUIDE.md` — workspace linking rules
5. `docs/rewrite/tasks/ADDON-SEAORM.md` — why the store layer moved to SeaORM
6. `CLAUDE.md` — workspace quality conventions

## Part 1 — Prior Audit Findings to Re-check

The prior audit produced these findings. For each one, determine whether
it is: **FIXED** (with evidence), **PARTIALLY FIXED**, **NOT FIXED**, or
**CONSCIOUSLY DEFERRED** (with a visible TODO, comment, or tracking link).
A deferred finding is acceptable only if the deferral is explicit in code
or docs — a silently-ignored finding is a regression.

### Prior Finding A — `[ISSUE]` Coverage gap: `status/enums.rs` at 0%

**What was wrong:** `Display::fmt` impls on the value enums (`DispatchMode`,
`StepType`, `Lane`, `Phase`, `Cli`, `AuthMode`, `Outcome`) plus `LeaseStatus`
and `StepStatus`/`StepReadyState` had zero test coverage. A `Display` typo
would slip past CI despite the 80%+ overall number.

**What to check:**
- Run `cargo llvm-cov nextest -p tanren-domain --summary-only` and verify
  `status/enums.rs` is at or near 100% line coverage.
- Every value enum should have at least one test asserting each variant's
  `Display` output matches its serde `snake_case` tag.
- Look for consistency: do `Display` outputs agree with the
  `#[serde(rename_all = "snake_case")]` values? A mismatch here would be a
  latent bug masquerading as fixed.

### Prior Finding B — `[ISSUE]` Envelope/payload timestamp duplication

**What was wrong:** `EventEnvelope::new(event_id, timestamp, payload)`
accepts a timestamp independent of `payload.timestamp`, and every
`DomainEvent` variant carries its own `timestamp` field with no enforced
relationship to the envelope's. No validation at decode time. Ambiguous
intent: occurrence time vs. persist time never documented.

**What to check:**
- Look for one of two resolutions:
  - **Resolution A (collapse):** `timestamp` has been removed from
    `DomainEvent` variants and derived exclusively from the envelope. Snapshot
    tests updated, `DomainEvent::timestamp()` accessor removed or redirects
    to the envelope-level timestamp.
  - **Resolution B (enforce):** `RawEventEnvelope::try_decode` now validates
    that `envelope.timestamp == payload.timestamp()` (or a documented
    tolerance), analogous to the existing `EntityMismatch` check. Doc
    comments clearly explain the semantic distinction.
- Either resolution is acceptable. **Neither resolution applied** is not.
- If a resolution was chosen, check the snapshot tests were updated and
  still pass. Check the envelope decode tests cover the new code path.

### Prior Finding C — `[ISSUE]` `EventEnvelope::new` locks all events to a dispatch root

**What was wrong:** `EventEnvelope::new` hardcodes
`EntityRef::Dispatch(payload.dispatch_id())` and `DomainEvent::dispatch_id()`
is infallible. The `EntityRef` enum already has `Org`, `Team`, `Project`
variants that are unreachable from the current construction path. When
the first non-dispatch event lands, this becomes a silent landmine.

**What to check:**
- This was rated as acceptable for Phase 0 with a TODO comment. Verify a
  `// TODO(phase-1):` or equivalent comment exists on
  `DomainEvent::dispatch_id()` (or somewhere visible) noting the
  non-dispatch event path requires adjustment.
- If no TODO exists, this finding is regressed from "acceptable" to
  "forgotten" and should be flagged.

### Prior Finding D — `[SUGGESTION]` Schema versioning policy not documented

**What was wrong:** `SCHEMA_VERSION: u32 = 1` has a one-line doc but no
policy for what constitutes a breaking change requiring a bump.

**What to check:**
- Is there a doc block (either in `events.rs`, a new file under `docs/`,
  or in CLAUDE.md) explaining:
  - What counts as forward-compatible (e.g., adding a variant with
    `#[serde(default)]` fields)?
  - What requires a version bump (renaming a field, removing a variant,
    changing a `#[serde(tag)]` value)?
  - What consumers must do on a version bump?
- If not, mark as still-open suggestion, not a blocker.

### Prior Finding E — `[SUGGESTION]` Output redaction contract

**What was wrong:** `ExecuteResult.tail_output`, `stderr_tail`, and
`gate_output` are free-form strings that could contain secrets leaked by
harnesses. The domain layer can't enforce redaction, but the spec and
code gave no guidance for Phase 1 harness authors.

**What to check:**
- Is there a `//!` doc block in `payloads.rs` (or on the `ExecuteResult`
  struct specifically) documenting that these fields are captured
  verbatim and that harness adapters are responsible for redaction?
- Does the Phase 1 harness lane brief carry the requirement forward?
  (If yes, this closes the finding; if no, the risk is still live.)

### Prior Finding F — `[SUGGESTION]` `DispatchFailed` vs `DispatchCompleted{Fail}` ambiguity

**What was wrong:** The domain schema allows two paths to "dispatch
finished unsuccessfully". The resolution was to enforce a single-path rule
in the orchestrator, not in the domain.

**What to check:**
- Verify LANE-0.4-CLI-WIRING.md carries the orchestrator-side rule
  (emit `DispatchCompleted` only for `Outcome::Success`; all non-success
  terminations use `DispatchFailed`; `DispatchCancelled` for user-initiated).
- The finding is closed if the rule is documented in the downstream brief.
  It is open if the rule is only in the audit notes.

## Part 2 — Regression Checks

These are things that were **correct** in the prior audit and must remain
correct. Catch any drift.

- **Secret redaction:** `ConfigEnv::Debug` still redacts values;
  `DispatchSnapshot` still uses `ConfigKeys` (not `ConfigEnv`);
  `EnvironmentHandle` still has no runtime metadata fields. Run the
  existing tests and verify they still pass verbatim.
- **State machine invariants:** Proptest suite for Dispatch/Step/Lease
  status still passes. No variant has been added without updating the
  proptest strategy.
- **Guard logic:** Every guard test still passes and still covers
  `allow_retry_after_failure` cancelled/running edge cases.
- **Envelope decode safety:** `RawEventEnvelope::try_decode` still rejects
  unsupported versions, unknown variants, and entity/payload mismatches.
- **Workspace linking rules:** `tanren-domain/Cargo.toml` has no internal
  workspace dependencies. `grep` for any `tanren-*` path deps in the
  domain crate — there should be zero.
- **No inline lint suppression:** `just check-suppression` passes.
- **File size discipline:** `just check-lines` passes (500-line cap).
- **`just ci` is green end-to-end.**

## Part 3 — SeaORM Compatibility Certification

The store layer (Lane 0.3) is shifting from raw `sqlx` to SeaORM. The
domain crate itself does not depend on SeaORM, sqlx, or any database
driver — and that must stay true. But the store layer will serialize
domain types into and out of SeaORM `JsonBinary` columns, which means
the domain types must round-trip cleanly through `serde_json::Value`
(not just `String`).

This is a **new audit dimension not covered in the original brief.**

### 3.1 — Dependency hygiene

- `cargo tree -p tanren-domain` must not contain `sea-orm`, `sea-orm-migration`,
  `sqlx`, `testcontainers`, or any database-specific crate.
- The domain `Cargo.toml` should depend only on: `chrono`, `serde`,
  `serde_json`, `thiserror`, `uuid`, plus dev-deps (`insta`, `proptest`).
- If the agent added any new dependency, verify it's justified and
  database-free.

### 3.2 — Value round-trip

The store will persist events via approximately this path:

```rust
// Write
let value: serde_json::Value = serde_json::to_value(&envelope)?;
// -> stored as JsonBinary (JSONB on Postgres, TEXT on SQLite)

// Read
let envelope: EventEnvelope = serde_json::from_value(value)?;
```

The existing snapshot tests verify `to_string` / `from_str` round-trip.
They do **not** currently verify `to_value` / `from_value`. These two
paths are structurally similar but differ in edge cases:

- `serde_json::Value` does not preserve object field order. JSONB on
  Postgres also does not preserve field order and deduplicates keys.
  Any domain type whose round-trip depends on key ordering is broken
  under SeaORM.
- `Value` goes through an intermediate untyped representation that
  exercises deserialization code paths not hit by the string path.
- Custom `Deserialize` impls (`NonEmptyString`, `TimeoutSecs`) must run
  their validators through the `from_value` path too.

**What to check:**

- Add or verify a test that round-trips every `DomainEvent` variant
  through `serde_json::to_value` → `serde_json::from_value::<EventEnvelope>`
  and asserts equality with the original. A single parameterized test
  or a loop over a `Vec<EventEnvelope>` is sufficient.
- If the test already exists, confirm it covers all variants and runs
  clean.
- Verify `NonEmptyString` and `TimeoutSecs` reject invalid JSON values
  through `from_value` (not just `from_str`). One test each is fine.
- Verify `DomainError::InvalidValue` is the observable error when the
  validators reject bad input during `from_value`.

### 3.3 — No JSONB-hostile field shapes

SeaORM's `JsonBinary` column type stores data as Postgres JSONB on
Postgres and TEXT on SQLite. JSONB has these properties:

- No preserved object field order
- Duplicate keys are removed (last wins)
- Whitespace is normalized
- Numeric precision: IEEE 754 double — **`u64` values above `2^53` lose
  precision on round-trip** through JSONB
- `f64::NaN` and `f64::INFINITY` are not representable in JSON at all

**What to check:**

- Scan the domain for `u64` fields used in event payloads. In the current
  code I can see: `TimeoutSecs(u64)`, `TokenUsage.{input_tokens,
  output_tokens, cache_read_tokens, cache_write_tokens}: u64`,
  `ResourceLimits.{max_memory_mb, max_cpu_millicores, max_disk_mb}: u64`,
  quota limits in errors. For each, ask: can a realistic production
  value ever exceed `2^53 ≈ 9.0e15`? Token counts, memory MB, and
  millicores will not. Timeouts in seconds will not (that's ~285M years).
  This is likely fine but should be explicitly verified by the auditor.
- Scan for `f64` fields: `duration_secs`, `total_duration_secs`,
  `estimated_cost`, `estimated_cost_hourly`, `BudgetExceeded.{limit, current}`.
  Verify there is no production path that could produce `NaN` or
  `Infinity`. If any exist, flag as a blocker — JSON serialization of
  non-finite floats fails.
- Scan for any field that relies on map key ordering for correctness.
  `ConfigKeys` is explicitly sorted, good. `ConfigEnv` is a `HashMap`
  but is command-only and not persisted to events — verify it does not
  appear in any event variant or `DispatchSnapshot`.

### 3.4 — Validated newtype re-validation on read

`NonEmptyString` and `TimeoutSecs` have custom `Deserialize` impls that
re-run their validators. This is correct and desirable — it means
corrupt rows in the database will fail cleanly at read time instead of
propagating invalid state.

**What to check:**

- Confirm that the validator error paths are still wired through `serde::de::Error::custom`.
- Confirm that attempting to `serde_json::from_value` a `NonEmptyString`
  from an empty string fails with a recognizable error message.
- Confirm the same for `TimeoutSecs` from `0`.
- This is a regression check: these tests existed in the original crate
  — verify they still pass.

### 3.5 — Chrono and UUID compatibility

SeaORM uses feature flags `with-chrono` and `with-uuid` to enable
`DateTime<Utc>` and `Uuid` column types. The domain types already use
these from `chrono` and `uuid` crates directly.

**What to check:**

- `chrono::DateTime<Utc>` round-trips through JSON as RFC 3339 strings.
  The existing snapshot tests confirm this — just verify they still pass.
- `uuid::Uuid` round-trips through JSON as hyphenated strings.
  `#[serde(transparent)]` on every ID newtype means they serialize as
  raw UUIDs. Verify by inspection that all ID types still use
  `#[serde(transparent)]`.

## Audit Process

1. Check out the branch under review. Confirm you're on the right branch
   (`lane-0.2` or its successor).
2. Run `just ci`. It must pass. If it doesn't, stop — the agent hasn't
   reached green state.
3. Run `cargo llvm-cov nextest -p tanren-domain --summary-only` and
   record the coverage numbers. Compare to the prior audit's baseline
   (77% line / 81% function overall, `status/enums.rs` at 0%).
4. Work through **Part 1** — go through each prior finding, determine
   its state, and collect evidence (file:line references).
5. Work through **Part 2** — run the regression checks.
6. Work through **Part 3** — verify SeaORM compatibility. This includes
   running any new round-trip tests the agent added and adding your own
   verification if they are missing.
7. For each finding, classify: **blocker** (must fix before lane ships),
   **issue** (should fix), **suggestion** (consider later),
   **not-applicable** (prior finding already resolved or irrelevant).
8. Produce the output report.

## Output Format

Deliver findings as a structured report with three sections:

### Section 1 — Prior Finding Status

A table of the six prior findings (A–F) with their current status:

```
Finding | Status         | Evidence
A       | FIXED          | status/enums.rs now 100% covered; see tests in …
B       | NOT FIXED      | envelope and payload still carry separate timestamps
C       | DEFERRED (OK)  | TODO comment at events.rs:345
...
```

### Section 2 — Regression and New Findings

Each finding formatted as:

```
### [BLOCKER|ISSUE|SUGGESTION] — Short title

**Dimension:** Correctness / Performance / Security / Maintainability /
              Scalability / SeaORM Compatibility
**Location:** crates/tanren-domain/src/<file>:<line>
**Finding:** Description of what's wrong and why it matters.
**Fix:** Concrete fix or direction.
```

### Section 3 — SeaORM Readiness Certification

One of:

- **CERTIFIED** — domain types are safe to use as SeaORM JSON column
  payloads. No concerns with field ordering, numeric precision, or
  round-trip fidelity.
- **CERTIFIED WITH CAVEATS** — safe with the specific caveats listed
  (e.g., "must avoid NaN for `f64` fields — responsibility falls on
  harness adapters in Phase 1").
- **NOT CERTIFIED** — specific domain types will not round-trip cleanly
  through SeaORM's JSON columns. List each type and explain.

### Section 4 — Recommendation

One of:

- **APPROVE for merge** — no blockers, findings are documented and tracked.
- **APPROVE with follow-up** — no blockers, but specific fixes should
  happen before the next lane begins. List them.
- **REJECT** — one or more blockers. List them and describe the fix path.
