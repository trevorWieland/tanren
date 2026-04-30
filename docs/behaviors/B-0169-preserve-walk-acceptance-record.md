---
schema: tanren.behavior.v0
id: B-0169
title: Preserve walk acceptance record after the walk
area: walk-acceptance
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can access a walk acceptance record after a walk so completed work remains auditable and understandable later.

## Preconditions

- A walk or acceptance review has produced an acceptance record.
- The user has visibility into the spec or project history.

## Observable outcomes

- Acceptance, rejection, demonstrations, checks, and notes remain linked to the spec.
- Walk acceptance records remain available after merge, cleanup, or later replanning.
- Redacted source references still show that hidden material existed and why it is hidden.

## Out of scope

- Retaining ephemeral runtime resources forever.
- Exposing secrets or hidden project details in walk acceptance views.

## Related

- B-0073
- B-0116
- B-0122
