---
schema: tanren.behavior.v0
id: B-0068
title: Bootstrap Tanren assets into an existing repository
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can bootstrap Tanren methodology assets
into an existing repository so that the project gains the commands, standards,
and agent integrations it needs to participate in the Tanren method.

## Preconditions

- The user has a repository where Tanren may install assets.
- The user chooses a supported standards profile.

## Observable outcomes

- A compatible Tanren project configuration exists or is created when none is
  present.
- The repository receives the methodology assets, agent integration assets,
  and standards required by the selected profile.
- Re-running install replaces generated assets, removes stale generated
  assets, preserves user-edited standards, and restores missing standards.
- Invalid bootstrap input is rejected before any assets are written.

## Out of scope

- Registering the repository as an account-level Tanren project.
- Importing repository history or project membership.
- Performing local repository bootstrap from phone-only interfaces.

## Related

- B-0025
- B-0049
- B-0069
- B-0070
