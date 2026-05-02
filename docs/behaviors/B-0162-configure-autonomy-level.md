---
schema: tanren.behavior.v0
id: B-0162
title: Configure Tanren's autonomy level
area: autonomy-controls
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can configure Tanren's autonomy level so automated work proceeds only as far as the user or policy allows.

## Preconditions

- An account, project, roadmap item, or spec scope is selected.
- The user has permission to configure automation policy for that scope.

## Observable outcomes

- The autonomy level is visible before Tanren starts affected work.
- Tanren explains which actions are automatic, approval-gated, or forbidden.
- Changed autonomy settings are attributed and traceable.

## Out of scope

- Encoding autonomy as a persona role.
- Overriding organization policy without permission.

## Related

- B-0002
- B-0115
- B-0163
