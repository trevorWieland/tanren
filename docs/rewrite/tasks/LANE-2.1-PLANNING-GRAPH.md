# Lane 2.1 — Planning Graph

> **Status:** Stub. Full brief to be written at the start of Phase 2.

## Scope

Implements planner-native orchestration in `tanren-planner` and
`tanren-scheduler`: task graph construction, dependency resolution,
replanning on failure, and scheduler-ready-state propagation.

## Carried-Forward Notes from Lane 0.2 Audit

### `GraphRevision` is present but unanchored

Lane 0.2 introduced the `GraphRevision(u32)` newtype
(`tanren_domain::graph::GraphRevision`) and threaded it through
commands, events, and views. The newtype provides:

- `GraphRevision::ZERO` / `GraphRevision::INITIAL`
- `next()` — saturating increment
- `is_stale_relative_to(current)` — stale-command detection
- `PartialOrd` / `Ord` for monotonic comparisons

**What this lane owns:** the orchestrator-side logic that rejects
stale `EnqueueStep` commands. A command arriving with
`graph_revision = 3` after the dispatch is at revision 5 must be
rejected with `DomainError::PreconditionFailed` (or a new dedicated
variant if more specificity helps projections).

**Decision point:** whether replanning advances the revision on every
re-plan or only on structural changes (additions, removals, reordered
dependencies). The current domain API is agnostic — both strategies
are expressible.

### Non-dispatch events and `entity_root()`

`DomainEvent::entity_root()` currently always returns
`EntityRef::Dispatch(self.dispatch_id())`. When Phase 2 introduces
org-level or team-level events (`OrgBudgetExhausted`,
`TeamQuotaReset`), each new variant should supply its own root
directly — and `DomainEvent::dispatch_id()` must either become
fallible (`Option<DispatchId>`) or be retired in favor of
`entity_root()`.

See the TODO on `DomainEvent::dispatch_id` in
`crates/tanren-domain/src/events.rs`.

## Dependencies

- Lane 0.2 (domain model)
- Lane 0.3 (store — event append, projection reads)
- Lane 0.4 (orchestrator wiring)
