---
schema: tanren.behavior.v0
id: B-0069
title: Detect installer drift without mutating files
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: asserted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can run install in strict dry-run mode so that
generated Tanren assets are checked for drift without changing the repository.

## Preconditions

- The repository has already been bootstrapped with Tanren assets.

## Observable outcomes

- Drift in generated command files produces a drift exit status.
- Missing preserved standards produce a drift exit status.
- Locally edited preserved standards are accepted.
- Strict dry-run leaves the repository's file contents unchanged.

## Out of scope

- Automatically applying drift remediation.
- Performing local repository drift checks from phone-only interfaces.

## Related

- B-0068
