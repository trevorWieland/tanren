# Tanren 2.0 — Implementation Task Briefs

## Phase 0: Foundation

### Current Status

| Lane | Crate(s) | Status | Notes |
|------|----------|--------|-------|
| 0.1 | workspace scaffold | ✅ merged | just tooling, lints, CI |
| 0.2 | `tanren-domain` | ✅ merged, audit-certified | canonical domain model frozen for downstream lanes |
| 0.3 | `tanren-store` | ✅ merged | foundation now carries the real store boundary |
| 0.4 | `tanren-contract`, `tanren-policy`, `tanren-orchestrator`, `tanren-app-services`, `tanren-observability`, `tanren-cli` | ✅ merged | dispatch CRUD slice merged into `rewrite/tanren-2-foundation` |
| 0.5 | methodology boundary docs + shared command markdown | 🔵 in progress on `lane-0.5` | separates tanren-code workflow mechanics from tanren-markdown agent behavior |

### Execution Order

```
Lane 0.1 (Workspace Scaffold) ✅ COMPLETE
         │
         ▼
Lane 0.2 (Domain Model) ✅ merged
         │
         ├────────────────────┬────────────────────┐
         ▼                    ▼                    ▼
Lane 0.3 (Store Core) ✅ merged   Lane 0.4 (Contract + CLI Wiring) ✅ merged   Lane 0.5 (Methodology Boundary) 🔵 in progress
         │                    │                    │
         └────────┬───────────┴───────────┬────────┘
                  ▼                       ▼
         Integration: contract/store CLI slice     Integration: manual self-hosting boundary before Phase 1
```

### Lane Details

| Lane | Crate(s) | Depends On | Status | Brief |
|------|----------|------------|--------|-------|
| 0.1 | workspace | — | ✅ Complete: scaffold, just, CI, lints |
| 0.2 | `tanren-domain` | 0.1 | ✅ merged, audit-certified | [LANE-0.2-DOMAIN.md](LANE-0.2-DOMAIN.md) |
| 0.3 | `tanren-store` | 0.2 | ✅ merged | [LANE-0.3-STORE.md](LANE-0.3-STORE.md) |
| 0.4 | `tanren-contract`, `tanren-policy`, `tanren-orchestrator`, `tanren-app-services`, `tanren-observability`, `tanren-cli` | 0.2 | ✅ merged | [LANE-0.4-CLI-WIRING.md](LANE-0.4-CLI-WIRING.md) |
| 0.5 | methodology boundary, typed task state, agent tool surface, multi-agent install, self-hosting (`tanren-domain::methodology`, `tanren-contract::methodology`, `tanren-store::methodology`, `tanren-app-services::methodology`, `tanren-mcp`, `commands/`) | 0.2, 0.3, 0.4 | 🔵 in progress on `lane-0.5` | [LANE-0.5-BRIEF.md](LANE-0.5-BRIEF.md) · [LANE-0.5-DESIGN-NOTES.md](LANE-0.5-DESIGN-NOTES.md) |

### First Milestone Exit Criteria

All of the following must be true:

- [ ] `just ci` passes across full workspace
- [ ] Domain model has comprehensive tests (state machines, serde round-trips, guards)
- [ ] Store implements EventStore + JobQueue + StateStore with SQLite
- [ ] `tanren dispatch create` → `tanren dispatch get` → `tanren dispatch list` works
- [ ] Event log is append-only and queryable
- [ ] Dispatch projections are consistent with event history
- [ ] Guard rules prevent invalid state transitions

### Parallelization Strategy

**Lane 0.2** ran first with one agent. **Lane 0.3** and **Lane 0.4** ran in
parallel worktrees and are now merged into foundation.

Dispatch rules:

- Implementation agents for the current lane get `LANE-0.5-BRIEF.md` plus `LANE-0.5-DESIGN-NOTES.md`
- Audit agents for the current lane get `LANE-0.5-AUDIT.md` plus `LANE-0.5-DESIGN-NOTES.md`
- Lane 0.5 scope is Phase-0 completion (methodology + typed task state + tool surface + install + self-hosting); it does not widen into Phase 1 harness/environment runtime work

Integration happens when both lanes merge: the CLI binary connects to the real store.

## Future Phases

Stubs below carry forward requirements from earlier audits so the
follow-up work is not lost. Full briefs will be fleshed out at the
start of each phase.

- **Phase 1**: Runtime substrate (harness traits, environment leases, worker)
  - [LANE-1.1-HARNESS.md](LANE-1.1-HARNESS.md) — harness contract + output redaction requirement
  - [LANE-1.2-RUNTIME.md](LANE-1.2-RUNTIME.md) — runtime substrate + `runtime_type` typing decision
- **Phase 2**: Planner-native orchestration (task graphs, scheduler, replanning)
  - [LANE-2.1-PLANNING-GRAPH.md](LANE-2.1-PLANNING-GRAPH.md) — graph revision enforcement + non-dispatch events
- **Phase 3**: Policy and governance (auth, budgets, placement)
- **Phase 4**: Interface parity (API, MCP, TUI)
- **Phase 5**: Scale and observability
- **Phase 6**: Migration and cutover
