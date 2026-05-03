---
kind: standard
name: three-tier-test-structure
category: testing
importance: high
applies_to:
  - "**/*test*"
  - "**/*spec*"
  - "tests/**"
applies_to_languages:
  - rust
applies_to_domains:
  - testing
---

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

## Tanren mapping: tiers are harness choice, not file location

In tanren, runtime tiers map to harness choice rather than a tier-tag or
a per-tier directory. The three tiers all share a single Gherkin source
tree:

| Tier | Harness | Selected by |
|------|---------|-------------|
| In-process | `tanren_app_services` handlers + ephemeral SQLite | (no interface tag — this is the unit substrate the in-process harness exposes; in tanren's BDD surface it appears only as the in-process implementation behind a per-interface harness adapter when relevant.) |
| Spawned-binary | `tanren-api-app` / `tanren-cli` / `tanren-mcp-app` / `tanren-tui` on ephemeral ports/pipes | `@api`, `@cli`, `@mcp`, `@tui` |
| Playwright | full browser against running api + Next.js dev server | `@web` |

`tests/bdd/features/B-XXXX-*.feature` is the **single source of truth**
for behavior scenarios. The `apps/web/tests/bdd/features/` location is a
symlink to that directory so that `playwright-bdd` (the Node-side
`@web` runner) consumes exactly the same files Rust does. There is no
duplication of scenario text between the Rust and TypeScript sides.

See `bdd-wire-harness.md` for the per-interface harness contract and
`mock-boundaries.md` § "Per-interface BDD wire harnesses (R-0001)" for
the rule that step definitions never call handlers directly.

**Why:** Tiered runtime control and single-format behavior proof can
coexist without ambiguity. Anchoring the tier to the harness rather than
a tag makes the runtime category mechanically obvious from the scenario
text.
