---
id: B-0071
title: Use the repository's installed standards
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: asserted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can rely on Tanren commands to use the standards
installed for the repository so that checks and guidance reflect the project's
chosen way of working.

## Preconditions

- The repository has Tanren support files installed.
- The repository has a configured standards location.

## Observable outcomes

- Commands that need standards succeed when the configured standards are
  present.
- Commands that need standards fail explicitly when the configured standards are
  missing.
- Missing standards are not silently replaced with unrelated fallback content
  during command execution.

## Out of scope

- Remote standards registries.
- Organization-level standards sync.

## Related

- B-0049
- B-0068
