---
id: B-0068
title: Bootstrap Tanren assets into an existing repository
personas: [solo-dev, team-dev]
interfaces: [cli]
contexts: [personal, organizational]
status: accepted
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can bootstrap Tanren support files into an
existing repository so that methodology commands, standards, and agent
integrations are installed from the bundled distribution.

## Preconditions

- The user has a local repository path where Tanren files may be written.
- The user chooses a supported standards profile.

## Observable outcomes

- `tanren.yml` is created when no compatible config exists.
- Methodology command assets, MCP config targets, and standards files are
  written for the selected profile and agent integrations.
- Re-running install replaces generated command files, removes stale generated
  command files, preserves edited standards, and restores missing standards.
- Invalid bootstrap input fails validation before bootstrap files are written.

## Out of scope

- Registering the repository as an account-level Tanren project.
- Importing repository history or project membership.

## Related

- B-0025
- B-0049
- B-0069
- B-0070

