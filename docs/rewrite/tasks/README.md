# Tanren 2.0 — Implementation Task Briefs

## Phase 0: Foundation

### Current Status

| Lane | Crate(s) | Status | Notes |
|------|----------|--------|-------|
| 0.1 | workspace scaffold | ✅ merged | just tooling, lints, CI |
| 0.2 | `tanren-domain` | ✅ merged, audit-certified | canonical domain model frozen for downstream lanes |
| 0.3 | `tanren-store` | ✅ merged | foundation now carries the real store boundary |
| 0.4 | `tanren-contract`, `tanren-policy`, `tanren-orchestrator`, `tanren-app-services`, `tanren-observability`, `tanren-cli` | ✅ merged | dispatch CRUD slice merged into `rewrite/tanren-2-foundation` |
| 0.5 | methodology boundary docs + shared command markdown | ✅ merged | separates tanren-code workflow mechanics from tanren-markdown agent behavior |

### Execution Order

```
Lane 0.1 (Workspace Scaffold) ✅ COMPLETE
         │
         ▼
Lane 0.2 (Domain Model) ✅ merged
         │
         ├────────────────────┬────────────────────┐
         ▼                    ▼                    ▼
Lane 0.3 (Store Core) ✅ merged   Lane 0.4 (Contract + CLI Wiring) ✅ merged   Lane 0.5 (Methodology Boundary) ✅ merged
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
| 0.5 | methodology boundary, typed task state, agent tool surface, multi-agent install, self-hosting (`tanren-domain::methodology`, `tanren-contract::methodology`, `tanren-store::methodology`, `tanren-app-services::methodology`, `tanren-mcp`, `commands/`) | 0.2, 0.3, 0.4 | ✅ merged + hardening follow-up scoped | [LANE-0.5-BRIEF.md](LANE-0.5-BRIEF.md) · [LANE-0.5-DESIGN-NOTES.md](LANE-0.5-DESIGN-NOTES.md) · [LANE-0.5-PHASE0-ENHANCEMENT-BRIEF.md](LANE-0.5-PHASE0-ENHANCEMENT-BRIEF.md) |

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

- Implementation agents for the current lane get `LANE-0.5-BRIEF.md` plus `LANE-0.5-DESIGN-NOTES.md`; Phase 0 hardening work uses `LANE-0.5-PHASE0-ENHANCEMENT-BRIEF.md`
- Audit agents for the current lane get `LANE-0.5-AUDIT.md` plus `LANE-0.5-DESIGN-NOTES.md`; Phase 0 hardening audit uses `LANE-0.5-PHASE0-ENHANCEMENT-AUDIT.md`
- Lane 0.5 scope is Phase-0 completion (methodology + typed task state + tool surface + install + self-hosting); it does not widen into Phase 1 harness/environment runtime work

Integration happened at the Phase 0 boundary: the CLI binary connects to the real store and the methodology/tool-surface lane is merged.

### Phase 0 Proof Closure

Phase 0 proof closure is now tracked and reproducible via:

- [../PHASE0_PROOF_BDD.md](../PHASE0_PROOF_BDD.md)
- [../PHASE0_PROOF_EVIDENCE_INDEX.md](../PHASE0_PROOF_EVIDENCE_INDEX.md)
- [../PHASE0_PROOF_RUNBOOK.md](../PHASE0_PROOF_RUNBOOK.md)
- `scripts/proof/phase0/run.sh` and `scripts/proof/phase0/verify.sh`

## Phase 1: Runtime Substrate (Planned)

Behavioral source of truth:

- [../PHASE1_PROOF_BDD.md](../PHASE1_PROOF_BDD.md)

### Lane Plan

| Lane | Area | Depends On | Status | Spec | Brief |
|------|------|------------|--------|------|-------|
| 1.1 | Harness contract | 0.5 | ✅ implemented on `rewrite/lane-1-1` (pending merge); contract-level guarantees only | [LANE-1.1-HARNESS.md](LANE-1.1-HARNESS.md) | [LANE-1.1-BRIEF.md](LANE-1.1-BRIEF.md) |
| 1.2 | Initial harness adapters (Claude Code, Codex, OpenCode) | 1.1 | ⏳ planned | [LANE-1.2-HARNESS-ADAPTERS.md](LANE-1.2-HARNESS-ADAPTERS.md) | [LANE-1.2-BRIEF.md](LANE-1.2-BRIEF.md) |
| 1.3 | Environment lease contract | 1.1, 1.2 | ⏳ planned | [LANE-1.3-ENV-CONTRACT.md](LANE-1.3-ENV-CONTRACT.md) | [LANE-1.3-BRIEF.md](LANE-1.3-BRIEF.md) |
| 1.4 | Initial environment adapters (local worktree + local-daemon containerized, DooD-ready constraints) | 1.3 | ⏳ planned | [LANE-1.4-ENV-ADAPTERS.md](LANE-1.4-ENV-ADAPTERS.md) | [LANE-1.4-BRIEF.md](LANE-1.4-BRIEF.md) |
| 1.5 | Worker runtime + proof closure | 1.2, 1.4 | ⏳ planned | [LANE-1.5-WORKER-RUNTIME.md](LANE-1.5-WORKER-RUNTIME.md) | [LANE-1.5-BRIEF.md](LANE-1.5-BRIEF.md) |

Legacy pointer retained for compatibility:

- [LANE-1.2-RUNTIME.md](LANE-1.2-RUNTIME.md)

### Execution Order

```
Lane 1.1 (Harness Contract)
         │
         ▼
Lane 1.2 (Harness Adapters)
         │
         ▼
Lane 1.3 (Environment Contract)
         │
         ▼
Lane 1.4 (Environment Adapters)
         │
         ▼
Lane 1.5 (Worker Runtime + Proof Closure)
```

### Phase 1 Exit Theme

By Phase 1 close, the same dispatch contract must run across multiple
harnesses and environments with normalized lifecycle/error semantics and
reproducible proof artifacts.

Lane boundary note: Lane 1.1 defines and hardens the harness contract and
conformance helpers; cross-harness semantic equivalence evidence for Feature 1,
Feature 3, and Feature 6 is accepted in Lane 1.2 when concrete adapters are
implemented.

## Future Phases (Beyond Phase 1)

- **Phase 2**: Planner-native orchestration (task graphs, scheduler, replanning)
  - [LANE-2.1-PLANNING-GRAPH.md](LANE-2.1-PLANNING-GRAPH.md) — graph revision enforcement + non-dispatch events
- **Phase 3**: Policy and governance (auth, budgets, placement)
- **Phase 4**: Interface parity (API, MCP, TUI)
- **Phase 5**: Scale and observability
- **Phase 6**: Migration and cutover
