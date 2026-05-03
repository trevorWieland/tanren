---
kind: standard
name: test-timing-rules
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

# Scenario Timing Rules

Per-scenario timing budgets keep the BDD suite usable as a feedback loop.
The runtime classification — fast vs slow — is implicit in the harness
chosen, not in a tag.

| Harness | Max Time | I/O Allowed |
|---------|----------|-------------|
| In-process (handlers + ephemeral SQLite) | 250 ms | No external |
| Spawned-binary (api/cli/mcp/tui on ephemeral ports/pipes) | 5 s | Yes |
| Cross-process Playwright (`@web` slice) | 10 s | Yes |

**Rules:**
- Per-interface harness selection (api/cli/mcp/tui/web) is documented in
  `bdd-wire-harness.md`; tier as a tag (`@tier(unit)`,
  `@tier(integration)`) is no longer used.
- Scenario timing budgets above are hard ceilings. Slow scenarios must be
  redesigned or split — there is no "slow" escape hatch.
- Nextest slow-timeout configuration still applies to Rust test binaries
  in CI as a backstop for runaway scenarios.
- No skip-based workaround for slow scenarios.

**Why:** Fast behavior feedback prevents test rot and gate avoidance.
Tying the budget to harness choice instead of an opt-in tag means timing
is enforced by construction.
