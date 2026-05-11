---
schema: tanren.behavior.v0
id: B-0134
title: Upgrade installed Tanren assets
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [cli]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can upgrade Tanren assets installed in a
repository so that generated assets advance to the current version while
user-owned files remain protected.

## Preconditions

- The repository has Tanren assets installed.
- The user has permission to modify repository support files.

## Observable outcomes

- The user can preview the upgrade before applying it.
- Confirmed apply produces exactly the writes and removals listed in the
  immediately-preceding preview.
- Generated assets update while preserved user-owned files remain protected.
- Compatibility or migration concerns are reported before destructive changes.

## Out of scope

- Upgrading external agent tools.
- Silently overwriting user-owned work.

## Verification evidence

- CLI: positive and falsification BDD scenarios exercising preview, confirmed
  apply, migration concern reporting, cancellation, user-file preservation,
  and no-install handling.

## Related

- B-0068
- B-0069
- B-0070
