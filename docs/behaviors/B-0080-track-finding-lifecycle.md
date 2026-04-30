---
schema: tanren.behavior.v0
id: B-0080
title: See unresolved check findings that block readiness
area: findings
personas: [solo-builder, team-builder]
interfaces: [cli, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: asserted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can see which check findings are still unresolved
so that readiness decisions are based on current source signals instead of stale
artifacts.

## Preconditions

- A spec is being checked by Tanren.
- A check can raise findings that require remediation before the work is ready.

## Observable outcomes

- New findings remain visible as unresolved until they are explicitly resolved,
  reopened, deferred, or superseded.
- Findings that require immediate remediation block readiness and check
  completion while they remain unresolved.
- Remediation work preserves links to the check, finding, investigation, and
  root-cause source signals that led to it.
- Implementation work can record repair source signals but cannot mark findings
  resolved by itself.
- Historical findings remain visible while readiness counts only unresolved
  blockers.

## Out of scope

- Automatically resolving findings solely because a later check passes.
- Scheduling or prioritizing deferred remediation.

## Related

- B-0003
- B-0006
- B-0021
