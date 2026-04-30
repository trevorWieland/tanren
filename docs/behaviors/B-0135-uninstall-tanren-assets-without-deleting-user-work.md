---
schema: tanren.behavior.v0
id: B-0135
title: Uninstall Tanren assets without deleting user work
area: project-setup
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can uninstall Tanren-managed assets from a repository so the repository can stop using Tanren without deleting user work.

## Preconditions

- The repository has Tanren assets installed.
- The user has permission to modify repository support files.

## Observable outcomes

- Generated Tanren assets can be removed deliberately.
- User-owned files, specs, and source signals are preserved unless explicitly exported or removed by separate action.
- The uninstall preview makes destructive effects visible before confirmation.

## Out of scope

- Deleting hosted account or project history.
- Removing external tracker issues or pull requests.

## Related

- B-0068
- B-0069
- B-0063
