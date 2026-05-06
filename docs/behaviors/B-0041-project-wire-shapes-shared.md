---
schema: tanren.behavior.v0
id: B-0041
title: Project wire shapes are shared across interfaces
area: cross-interface
personas: [solo-builder, team-builder, integration-client]
interfaces: [api, mcp, cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

Project request and response wire shapes (spec views, dependency views,
parameter structs, and failure bodies) are defined in `tanren-contract` and
shared identically by the api, mcp, and cli surfaces so every transport
exposes the same serialization contract.

## Preconditions

- A connected project exists.
- An account has visibility over the project.

## Observable outcomes

- Connecting, listing, disconnecting, listing specs, and listing
  dependencies produce the same JSON shape through api, mcp, and cli.
- Failure responses follow the shared `{code, summary}` taxonomy owned
  by contract helpers.

## Out of scope

- tui and web interface coverage (those surfaces reuse the same contract
  types but are proven in their own behavior slices).
- Persistence semantics, reconnect semantics, or provider modeling.

## Related

- B-0030
- B-0183
