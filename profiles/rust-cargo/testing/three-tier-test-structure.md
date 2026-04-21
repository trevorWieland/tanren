# Three-Tier BDD Structure

Rust tests are organized by runtime tier, but all executable behavior proof is scenario-driven.

```
crates/myapp-core/
├── tests/
│   ├── unit/
│   │   ├── features/
│   │   │   └── scheduler.feature
│   │   └── steps/
│   │       └── scheduler_steps.rs
│   ├── integration/
│   │   ├── features/
│   │   │   └── dispatch_flow.feature
│   │   └── steps/
│   │       └── dispatch_flow_steps.rs
│   └── support/
│       └── common.rs
```

**Tier definitions:**
- Unit scenarios: isolated behavior, no external I/O
- Integration scenarios: public API behavior with real service boundaries
- Spec-level scenarios: end-to-end behavior across crate boundaries

**Rules:**
- Every scenario includes `@behavior(BEH-...)`
- No standalone executable tests that are not mapped to scenarios
- Keep helper code in support modules, not as free-form untracked tests

**Why:** Tiered runtime control and single-format behavior proof can coexist without ambiguity.
