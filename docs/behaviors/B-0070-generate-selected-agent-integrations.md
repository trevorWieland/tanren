---
schema: tanren.behavior.v0
id: B-0070
title: Generate selected agent integrations deterministically
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can restrict Tanren installation to selected
agent integrations so that the repository only receives the integration assets
the user requested.

## Preconditions

- The user bootstraps a repository with a supported standards profile.
- The user names one or more supported agent integrations.

## Observable outcomes

- Only the selected agent integrations receive their assets.
- Unselected agent integrations receive no assets.
- Standards required by the selected profile are still installed.

## Out of scope

- Configuring agents after installation through account or organization
  settings.
- Performing local repository installation from phone-only interfaces.

## Related

- B-0068
