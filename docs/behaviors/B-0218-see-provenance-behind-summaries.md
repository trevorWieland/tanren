---
schema: tanren.behavior.v0
id: B-0218
title: See provenance behind summaries
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see provenance behind summaries so Tanren distinguishes source-backed status from uncertain interpretation.

## Preconditions

- A visible summary, forecast, recommendation, or report includes interpreted status.
- Tanren has source signals, missing source signals, or uncertainty that affects the interpretation.

## Observable outcomes

- Tanren shows provenance, uncertainty, and source signal limits in user-visible terms.
- Insufficiently supported summaries identify what source signals are missing or stale.
- Users can navigate from provenance statements to supporting source references where visible.

## Out of scope

- Pretending provenance is a mathematically precise guarantee.
- Hiding low provenance to make reports look cleaner.

## Related

- B-0207
- B-0209
- B-0219
