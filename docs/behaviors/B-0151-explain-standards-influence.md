---
id: B-0151
title: Explain which standards influenced a decision
area: standards-evolution
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see which standards influenced a planning or execution decision so Tanren's guidance remains explainable.

## Preconditions

- A planning, shaping, execution, or review decision references project standards.
- The user has visibility into the decision and relevant standards.

## Observable outcomes

- The decision links to the standards that affected it.
- The explanation names the user-visible reasoning, not internal implementation details.
- Missing or outdated standards are called out when they affect confidence.

## Out of scope

- Exposing standards outside the user's visible scope.
- Treating standards as the only source of decision rationale.

## Related

- B-0071
- B-0098
- B-0112
