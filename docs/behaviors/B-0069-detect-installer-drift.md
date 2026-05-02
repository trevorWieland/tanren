---
schema: tanren.behavior.v0
id: B-0069
title: Detect installer drift without mutating files
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can check whether installed Tanren assets
match the current standards profile without modifying the repository, so that
drift is visible before any update.

## Preconditions

- The repository has already been bootstrapped with Tanren assets.

## Observable outcomes

- Drift in generated assets is reported.
- Missing preserved standards are reported as drift.
- User-edited preserved standards are accepted as non-drift.
- The drift check leaves the repository unchanged.

## Out of scope

- Automatically applying drift remediation.
- Performing local repository drift checks from phone-only interfaces.

## Related

- B-0068
