---
schema: tanren.behavior.v0
id: B-0156
title: Block readiness for untestable behavior
area: spec-quality
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can be prevented from marking untestable behavior ready so Tanren does not execute work without assertable outcomes.

## Preconditions

- A spec is being reviewed for readiness.
- Project policy requires behavior to be testable before readiness.

## Observable outcomes

- Tanren identifies the readiness concern in user-visible terms.
- The spec remains not ready until the concern is resolved or explicitly overridden when policy allows.
- The user can see what source signals or acceptance criteria would make the behavior testable.

## Out of scope

- Requiring implementation-specific test designs during shaping.
- Blocking exploratory product discovery from remaining in draft form.

## Related

- B-0019
- B-0076
- B-0157
