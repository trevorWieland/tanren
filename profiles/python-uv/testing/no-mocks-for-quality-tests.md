# No Model Mocks in Quality Scenarios

Quality-tier BDD scenarios validate real model behavior. Model mocks are forbidden in this tier.

```gherkin
@behavior(BEH-QUAL-001) @tier(quality)
Scenario: Summary quality with production model
  Given a real model adapter with valid credentials
  When summary generation runs
  Then the output includes all required sections
```

```gherkin
@behavior(BEH-INT-001) @tier(integration)
Scenario: Pipeline orchestration without real model cost
  Given a mocked model boundary
  When the pipeline executes
  Then orchestration succeeds and outputs are persisted
```

**Rules:**
- Quality tier: real model adapters only
- Integration tier: mocked model boundary allowed
- Unit tier: mocked model boundary required
- Quality scenarios failing due to model behavior must be fixed, not bypassed

**Why:** Quality behavior claims are only trustworthy if executed against real model behavior.
