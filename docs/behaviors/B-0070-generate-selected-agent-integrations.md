---
id: B-0070
title: Generate selected agent integrations deterministically
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: asserted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can restrict installer output to selected agent
integrations so that the repository only receives the command and config
targets the user requested.

## Preconditions

- The user bootstraps a repository with a supported standards profile.
- The user names one or more supported agent integrations.

## Observable outcomes

- Only the selected agent command and MCP config targets are written.
- Unselected agent command and config targets are not written.
- Standards required by the selected profile are still installed.

## Out of scope

- Configuring agents after installation through account or organization
  settings.

## Related

- B-0068
