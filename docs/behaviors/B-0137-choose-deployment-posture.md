---
schema: tanren.behavior.v0
id: B-0137
title: Choose a deployment posture
area: project-setup
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can choose whether Tanren is used through hosted, self-hosted, or local-only infrastructure so project work runs in an acceptable trust boundary.

## Preconditions

- The user has permission to configure deployment posture for the account, project, or installation.

## Observable outcomes

- The selected posture is visible before project work is dispatched.
- Tanren explains which capabilities are available or unavailable for the posture.
- Later runtime and credential choices inherit the selected posture unless changed with permission.

## Out of scope

- Provisioning specific infrastructure providers.
- Hard-coding posture choices to personas.

## Related

- B-0102
- B-0108
- B-0136
