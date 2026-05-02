---
schema: tanren.behavior.v0
id: B-0209
title: See what supports a summary and how trustworthy it is
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: [B-0218]
---

## Intent

A user can see what supports a status summary and how trustworthy that
support is, so dashboards, digests, forecasts, and reports remain reviewable
rather than opaque.

## Preconditions

- A visible dashboard, digest, forecast, or report includes a summary.
- The user has visibility into at least some of the supporting source
  references.

## Observable outcomes

- Summary claims link to the specs, graph nodes, checks, reviews, findings,
  decisions, or outcomes behind them.
- The user can distinguish source-backed claims from inferred or interpreted
  text, and see which source signals are missing or stale.
- Hidden source signals are redacted without removing the fact that redaction
  affected the summary.

## Out of scope

- Exposing secret values or hidden project details.
- Requiring every summary to be manually authored.
- Pretending uncertainty is a mathematically precise guarantee.

## Related

- B-0116
- B-0169
- B-0207
