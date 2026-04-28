---
id: B-0171
title: Preserve rejected alternatives
area: decision-memory
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can preserve rejected alternatives so Tanren does not repeatedly reopen decisions without new evidence.

## Preconditions

- A planning, shaping, standards, or review decision has alternatives.
- The user has permission to record decision context.

## Observable outcomes

- Rejected alternatives are recorded with the reason for rejection.
- Future recommendations can reference rejected alternatives when relevant.
- New evidence can reopen a rejected alternative without erasing the prior decision.

## Out of scope

- Hiding alternatives that affect safety or product fit.
- Blocking all reconsideration of rejected ideas.

## Related

- B-0158
- B-0170
- B-0173
