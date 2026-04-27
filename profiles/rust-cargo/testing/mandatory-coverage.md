---
kind: standard
name: mandatory-coverage
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

# Coverage from Behavior Scenarios

Coverage must be interpreted through behavior scenarios, not isolated line-count targets.

```bash
# Example: collect coverage from scenario-driven execution
cargo llvm-cov nextest --workspace --lcov --output-path lcov.info
```

**Rules:**
- Coverage discussion is behavior-first: which behavior scenarios are missing?
- Behavior witness coverage and Rust source coverage are separate report
  sections
- CLI and MCP subprocess coverage must be intentionally captured before a
  report claims product-code reachability
- Uncovered code must be classified as:
  - missing behavior scenario
  - dead/removable code
  - non-scenario support code with explicit rationale
- Coverage thresholds remain required, but they are secondary to scenario completeness
- Do not claim behavior support without scenario evidence

**Why:** Coverage is useful when it highlights unproven behavior, not when treated as a vanity metric.
