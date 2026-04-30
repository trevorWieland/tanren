---
schema: tanren.behavior.v0
id: B-0277
title: See behavior coverage and verification status
area: product-planning
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see behavior coverage and verification status so product planning distinguishes intended behavior from implemented or asserted behavior.

## Preconditions

- A behavior catalog exists.
- The user has visibility into the selected product or project planning context.

## Observable outcomes

- The user can see which behaviors are accepted, draft, deprecated, removed, unimplemented, implemented, asserted, or retired.
- The view distinguishes product acceptance from verification status.
- Missing, stale, or uncertain status is visible without treating implementation source signals as part of the behavior contract.

## Out of scope

- Proving a behavior directly from the catalog view.
- Embedding implementation-specific source references in behavior files.
- Treating verification status as a substitute for product acceptance.

## Related

- B-0116
- B-0209
- B-0218
- B-0276
