---
id: B-0069
title: Detect installer drift without mutating files
personas: [solo-dev, team-dev]
interfaces: [cli]
contexts: [personal, organizational]
status: accepted
supersedes: []
---

## Intent

A `solo-dev` or `team-dev` can run install in strict dry-run mode so that
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

## Related

- B-0068

