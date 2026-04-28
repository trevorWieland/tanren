---
id: B-0169
title: Preserve acceptance evidence after the walk
area: walk-evidence
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can access acceptance evidence after a walk so completed work remains auditable and understandable later.

## Preconditions

- A walk or acceptance review has produced evidence.
- The user has visibility into the spec or project history.

## Observable outcomes

- Acceptance, rejection, demonstrations, checks, and notes remain linked to the spec.
- Evidence remains available after merge, cleanup, or later replanning.
- Redacted evidence still shows that hidden material existed and why it is hidden.

## Out of scope

- Retaining ephemeral runtime resources forever.
- Exposing secrets or hidden project details in evidence views.

## Related

- B-0073
- B-0116
- B-0122
