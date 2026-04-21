# Scenario Timing Rules

Timing limits apply to BDD scenario execution tiers.

| Tier | Max Time | I/O Allowed |
|------|----------|-------------|
| Unit scenario | 250ms | No |
| Integration scenario | 5s | Yes |
| Doc/API scenario examples | 250ms | No |

**Rules:**
- Scenario tier tagging must match runtime behavior
- Slow scenarios must be split or moved to the correct tier
- Nextest slow-timeout configuration still applies to Rust test binaries in CI
- No skip-based workaround for slow scenarios

**Why:** Fast behavior feedback prevents test rot and gate avoidance.
