---
schema: tanren.behavior.v0
id: B-0068
title: Bootstrap Tanren assets into an existing repository
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: asserted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can bootstrap Tanren support files into an
existing repository so that methodology commands, standards, and agent
integrations are installed from the bundled distribution.

## Preconditions

- The user has a local repository path where Tanren files may be written.
- The user chooses a supported standards profile.

## Observable outcomes

- A compatible Tanren project configuration exists or is created when none is
  present.
- Methodology command assets, MCP config targets, and standards files are
  written for the selected profile and agent integrations.
- Re-running install replaces generated command files, removes stale generated
  command files, preserves edited standards, and restores missing standards.
- Invalid bootstrap input fails validation before bootstrap files are written.

## Out of scope

- Registering the repository as an account-level Tanren project.
- Importing repository history or project membership.
- Performing local repository bootstrap from phone-only interfaces.

## Related

- B-0025
- B-0049
- B-0069
- B-0070
