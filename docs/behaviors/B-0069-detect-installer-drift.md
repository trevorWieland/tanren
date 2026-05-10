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
- The operator runs `tanren-cli drift --profile <PROFILE> [--repo <PATH>] [--integrations <CSV>]`.

## Observable outcomes

- The command emits a stable two-line summary contract:
  - Line 1 fields:
    `status=<ok|drift> command=drift ... clean=<u32> changed_generated=<u32> missing_generated=<u32> missing_preserved=<u32> accepted_preserved=<u32> drift=<u32>`.
  - Line 2 fields:
    `paths clean=[...] changed_generated=[...] missing_generated=[...] missing_preserved=[...] accepted_preserved=[...]`.
- A clean repository returns `status=ok command=drift` with `drift=0`.
- Drifted generated assets are reported under `changed_generated` or
  `missing_generated`, with `status=drift command=drift`.
- Missing preserved standards are reported under `missing_preserved`, with
  `status=drift command=drift`.
- User-edited preserved standards are accepted as non-drift and reported under
  `accepted_preserved` with `status=ok command=drift`.
- Path lists in the drift report are repository-relative.
- The drift check leaves the repository unchanged.

## Out of scope

- Automatically applying drift remediation (owned by install in R-0023).
- Applying upgrade-time migration/remediation behavior (owned by R-0026).
- Performing local repository drift checks from phone-only interfaces.

## Related

- B-0068
