# Coverage from Behavior Scenarios

Coverage must be interpreted through behavior scenarios, not isolated line-count targets.

```bash
# Example: collect coverage from scenario-driven execution
cargo llvm-cov nextest --workspace --lcov --output-path lcov.info
```

**Rules:**
- Coverage discussion is behavior-first: which behavior scenarios are missing?
- Uncovered code must be classified as:
  - missing behavior scenario
  - dead/removable code
  - non-scenario support code with explicit rationale
- Coverage thresholds remain required, but they are secondary to scenario completeness
- Do not claim behavior support without scenario evidence

**Why:** Coverage is useful when it highlights unproven behavior, not when treated as a vanity metric.
