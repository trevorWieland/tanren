# Scenario Timing Rules

Timing limits apply per executed scenario.

**Limits:**
- Unit scenarios: <250ms
- Integration scenarios: <5s
- Quality scenarios: <30s

**Rules:**
- Tier tags define expected runtime (`@tier(unit|integration|quality)`)
- Slow scenarios must be split or moved to the correct tier
- CI surfaces slowest scenarios and fails repeated violations
- Do not create a "slow" loophole marker to bypass these limits

**Why:** Fast, deterministic scenario feedback keeps behavior validation usable in daily development.
