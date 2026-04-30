---
schema: tanren.behavior.v0
id: B-0263
title: Report external CI or source-control status
area: integration-contract
personas: [integration-client, solo-builder, team-builder, observer]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `integration-client` can report external CI or source-control status so Tanren work reflects review and build state outside Tanren.

## Preconditions

- The client is authenticated and permitted to report status for the project or work item.
- The external status references a known commit, pull request, spec, graph node, or integration resource.

## Observable outcomes

- Reported status links to the relevant Tanren work and external resource.
- Users can distinguish pending, passing, failing, cancelled, and unavailable external status.
- Conflicting or stale status reports are visible rather than silently replacing newer source signals.

## Out of scope

- Treating external status as user acceptance.
- Trusting unauthenticated status reports.

## Related

- B-0118
- B-0229
- B-0259
