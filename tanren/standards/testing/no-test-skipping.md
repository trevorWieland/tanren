---
kind: standard
name: no-test-skipping
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

# No Scenario Skipping

No ignored tests and no skipped scenarios. Behavior evidence requires full execution.

**Rules:**
- No `#[ignore]`, no conditional early-return skips
- No feature-flag loophole that silently excludes behavior scenarios
- If a scenario is flaky, fix determinism or remove it with behavior-map updates
- CI fails on any skip-like suppression

**Why:** Unexecuted scenarios provide zero behavior proof.
