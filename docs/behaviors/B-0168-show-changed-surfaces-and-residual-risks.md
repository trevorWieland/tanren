---
schema: tanren.behavior.v0
id: B-0168
title: Show changed surfaces and residual risks
area: walk-acceptance
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see changed surfaces and residual risks before acceptance so they understand the blast radius of delivered work.

## Preconditions

- Work has produced changes or source signals for a spec.
- The user has visibility into the affected project surfaces.

## Observable outcomes

- Tanren summarizes user-facing, API, data, configuration, or operational surfaces that changed.
- Residual risks and known limitations are visible before acceptance.
- Risks can be accepted, rejected, or routed to follow-up work.

## Out of scope

- Listing every internal file or code symbol by default.
- Treating unknown risk as zero risk.

## Related

- B-0072
- B-0116
- B-0124
