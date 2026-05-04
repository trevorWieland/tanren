---
kind: standard
name: test-tooling
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

# Test Tooling for BDD-Only Rust

`cucumber-rs` is the canonical behavior runner. Supporting tools strengthen scenario quality.

**Core tooling:**
- `cucumber-rs`: executable Gherkin behavior scenarios
- `wiremock`: network boundary simulation in integration scenarios
- `testcontainers`: real service integration scenarios
- `insta`: snapshot assertions inside step implementations when output shape is complex
- `proptest`: invariant/property checks invoked from scenario-bound step code where needed

**Modern 2026 stack additions (R-0001):**

| Crate / package | Pinned version | Used for |
|---|---|---|
| `cucumber` | `0.23` | Existing canonical Rust BDD runner |
| `playwright-bdd` | latest | `@web` slice only — Node-side harness that consumes the same `.feature` files via symlink |
| `expectrl` | `0.7` | `@tui` BDD wire scenarios — interactive PTY-driven step coverage of `bin/tanren-tui` |
| `portable-pty` | `0.8` | PTY allocation underlying `expectrl` (cross-platform) |
| `wiremock` | `0.6` | HTTP boundary fakes for outbound integrations |
| `rstest` | `0.23` | Parameterized tests if needed (still rare; BDD covers most cases) |

`expectrl` + `portable-pty` together let `@tui` scenarios drive the real
`tanren-tui` binary through a pseudoterminal so keystrokes, resize, and
ANSI-rendered output are all part of the witness. `playwright-bdd` is
the only Node-side BDD tool in the stack; the Rust side stays on
`cucumber-rs`.

```gherkin
@behavior @dispatch @cli
Feature: Dispatch event persistence

  @B-0123 @positive
  Scenario: Dispatch event is persisted and replayable
  Given a running event store
  When a dispatch is created
  Then replay returns the created dispatch event
```

**Rules:**
- Behavior assertions start from `.feature` scenarios
- Active behavior features are discovered recursively under `tests/bdd/features`
- Supporting tools may be used in step implementations, not as replacement for scenario coverage
- Scenario tags must include stable `B-XXXX` behavior IDs and witness tags
- CLI behavior scenarios execute built binaries through explicit paths in
  normal runs and through the mutated workspace during mutation runs

**Why:** One behavior format with focused supporting tools keeps tests both readable and technically strong.
