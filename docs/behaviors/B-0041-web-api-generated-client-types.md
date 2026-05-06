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

The web frontend's account client uses TypeScript types generated from
the API's OpenAPI specification rather than handwritten duplicates, so
the client and server wire contract stays in sync without manual
maintenance.

## Preconditions

- The API server exposes an OpenAPI spec at `/openapi.json`.
- The `openapi-typescript` tool is configured in the web workspace.

## Observable outcomes

- Organization and project request/response shapes are imported from
  the generated type module.
- No handwritten interface duplicates of `OrganizationSwitcher`,
  `ProjectView`, or `ListOrganizationProjects` exist in the client
  module.
- The web client successfully lists organizations, switches the active
  organization, and lists projects using the generated types.

## Out of scope

- UI layout or state management changes.
- Generation pipeline automation (CI hook).

## Related

- B-0043
- B-0047
