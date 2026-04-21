# BDD Across All Test Tiers

All executable tests are behavior tests and must be authored as Gherkin scenarios. There is no non-BDD test tier in CI.

```gherkin
Feature: Pipeline execution

  @behavior(BEH-PROC-001) @tier(unit)
  Scenario: Processor rejects unsupported format
    Given a processor configured for "json" input
    When input format is "xml"
    Then processing is rejected with error code "unsupported_format"

  @behavior(BEH-PROC-002) @tier(integration)
  Scenario: Pipeline stores normalized result
    Given a configured pipeline with sample data
    When the pipeline runs to completion
    Then the normalized result is written to storage

  @behavior(BEH-PROC-003) @tier(quality)
  Scenario: Prompted summary satisfies quality threshold
    Given a production prompt and a real model adapter
    When summary generation completes
    Then the summary contains required sections
```

**Rules:**
- Use `pytest-bdd` for unit, integration, and quality tiers
- Every scenario must include a stable behavior tag: `@behavior(BEH-...)`
- Every scenario must include exactly one tier tag: `@tier(unit|integration|quality)`
- Step implementations may call helper functions, but scenario behavior is the source of truth
- If a behavior exists, it must exist as at least one scenario

**Why:** A single executable format keeps behavioral intent, implementation checks, and review language aligned.
