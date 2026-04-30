---
schema: tanren.behavior.v0
id: B-0207
title: See delivery forecast and risk
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see delivery forecast and risk so expectations are grounded in current source signals rather than optimism.

## Preconditions

- The selected scope has planned work and enough source signals to support a forecast or risk statement.
- The user has visibility into the relevant planning and delivery context.

## Observable outcomes

- Tanren presents forecast provenance and major risk drivers.
- The forecast distinguishes source-backed projection from missing or uncertain data.
- Changes to risk or forecast can be traced to recent work, blockers, replans, or outcomes.

## Out of scope

- Guaranteeing delivery dates.
- Producing forecasts from hidden or unavailable source signals without disclosure.

## Related

- B-0035
- B-0113
- B-0218
