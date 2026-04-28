---
id: B-0152
title: Detect conflicting standards before work starts
area: standards-evolution
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can detect conflicting standards before work starts so Tanren does not execute against incompatible guidance.

## Preconditions

- An active project has standards or methodology configuration.
- The user has visibility into standards that apply to the work.

## Observable outcomes

- Conflicting standards are shown with the work they affect.
- Tanren identifies whether the conflict blocks readiness or only lowers confidence.
- The user can route the conflict to standards revision or explicit override when policy allows.

## Out of scope

- Resolving standards conflicts without user or policy approval.
- Treating implementation preferences as product behavior.

## Related

- B-0049
- B-0149
- B-0157
