---
schema: tanren.behavior.v0
id: B-0041
title: Web API client consumes OpenAPI-generated TypeScript types
area: governance
personas: [solo-builder, team-builder]
interfaces: [web, api]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

The web frontend's account client imports organization and project
request/response shapes from the OpenAPI-generated type module instead
of maintaining handwritten duplicates. The generated types ensure the
client and server wire contract stays in sync without manual maintenance.

## Preconditions

- The API server is running with a valid backing store.
- An account exists with organization memberships.

## Observable outcomes

- The web client lists organizations using generated OpenAPI types.
- The web client switches the active organization using generated types.
- The web client lists projects using generated types.
- Error responses are parsed via a shared response/error parser shared
  between `postJson` and `getJson`.

## Out of scope

- Generating the OpenAPI schema itself (that is owned by the API binary).
- UI state or layout concerns.

## Related

- B-0047
- B-0043
