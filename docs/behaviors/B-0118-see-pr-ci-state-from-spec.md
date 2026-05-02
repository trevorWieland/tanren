---
schema: tanren.behavior.v0
id: B-0118
title: See pull request and CI state from the spec
area: review-merge
personas: [solo-builder, team-builder, observer, integration-client]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see pull request and CI state from the spec so source-control review does not disappear from Tanren context.

## Preconditions

- The spec has an associated pull request.
- The user has visibility of the spec.

## Observable outcomes

- The spec view shows pull request state.
- The spec view shows CI state when available.
- The user can navigate to the external pull request when authorized.

## Out of scope

- Replacing the source-control provider UI.
- Editing CI configuration.

## Related

- B-0057
- B-0117
- B-0119
