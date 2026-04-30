---
schema: tanren.behavior.v0
id: B-0209
title: See source signals behind a status summary
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see source signals behind a status summary so dashboards and reports are reviewable rather than opaque.

## Preconditions

- A visible dashboard, digest, forecast, or report includes a status summary.
- The user has visibility into at least some supporting source references.

## Observable outcomes

- Summary claims link to the specs, graph nodes, checks, reviews, findings, decisions, or outcomes behind them.
- Hidden source signals are redacted without removing the fact that redaction affected the summary.
- The user can distinguish direct source signals from inferred summary text.

## Out of scope

- Exposing secret values or hidden project details.
- Requiring every summary to be manually authored.

## Related

- B-0116
- B-0169
- B-0218
