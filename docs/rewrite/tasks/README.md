# Tanren 2.0 — Implementation Task Briefs

## Phase 0: Foundation

### Execution Order

```
Lane 0.1 (Workspace Scaffold) ✅ COMPLETE
         │
         ▼
Lane 0.2 (Domain Model)       ← START HERE — sequential, blocks everything
         │
         ├────────────────────┐
         ▼                    ▼
Lane 0.3 (Store Core)    Lane 0.4 (Contract + CLI Wiring)
         │                    │
         └────────┬───────────┘
                  ▼
         Integration: CLI creates/queries dispatches via store
```

### Lane Details

| Lane | Crate(s) | Depends On | Brief |
|------|----------|------------|-------|
| 0.1 | workspace | — | ✅ Complete: scaffold, just, CI, lints |
| 0.2 | `tanren-domain` | 0.1 | [LANE-0.2-DOMAIN.md](LANE-0.2-DOMAIN.md) |
| 0.3 | `tanren-store` | 0.2 | [LANE-0.3-STORE.md](LANE-0.3-STORE.md) |
| 0.4 | `tanren-contract`, `tanren-policy`, `tanren-orchestrator`, `tanren-app-services`, `tanren-observability`, `tanren-cli` | 0.2 | [LANE-0.4-CLI-WIRING.md](LANE-0.4-CLI-WIRING.md) |

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

**Lane 0.2** runs first with one agent. Once domain types compile and tests pass,
**Lane 0.3** and **Lane 0.4** launch in parallel worktrees:

- Lane 0.3 agent works in `crates/tanren-store/` — needs domain types but not CLI/contract
- Lane 0.4 agent works in `crates/tanren-{contract,policy,orchestrator,app-services,observability}/` and `bin/tanren-cli/` — needs domain types but can mock store traits

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
