---
id: B-0134
title: Upgrade installed Tanren assets
area: project-setup
personas: [solo-builder, team-builder, operator]
interfaces: [cli, api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can upgrade Tanren assets installed in a repository so command and standards support can move forward deliberately.

## Preconditions

- The repository has Tanren assets installed.
- The user has permission to modify repository support files.

## Observable outcomes

- The user can preview the upgrade before applying it.
- Generated assets update while preserved user-owned files remain protected.
- Compatibility or migration concerns are reported before destructive changes.

## Out of scope

- Upgrading external agent tools.
- Silently overwriting user-owned work.

## Related

- B-0068
- B-0069
- B-0070
