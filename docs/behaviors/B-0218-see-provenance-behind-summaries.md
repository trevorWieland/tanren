---
id: B-0218
title: See confidence behind summaries
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see confidence behind summaries so Tanren distinguishes evidence-backed status from uncertain interpretation.

## Preconditions

- A visible summary, forecast, recommendation, or report includes interpreted status.
- Tanren has evidence, missing evidence, or uncertainty that affects the interpretation.

## Observable outcomes

- Tanren shows confidence, uncertainty, and evidence limits in user-visible terms.
- Low-confidence summaries identify what evidence is missing or stale.
- Users can navigate from confidence statements to supporting evidence where visible.

## Out of scope

- Pretending confidence is a mathematically precise guarantee.
- Hiding low confidence to make reports look cleaner.

## Related

- B-0207
- B-0209
- B-0219
