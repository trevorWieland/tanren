---
schema: tanren.behavior.v0
id: B-0178
title: Prepare release notes from accepted work
area: release-learning
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can prepare release notes from accepted work so shipped changes are communicated from product source signals.

## Preconditions

- Accepted or merged work exists for a release scope.
- The user has visibility into the work and permission to prepare release material.

## Observable outcomes

- Tanren drafts release notes linked to accepted specs and source signals.
- User-visible changes, fixes, risks, and follow-up work are distinguishable.
- The user can revise release notes before they are published or exported.

## Out of scope

- Publishing to external channels without configured permission.
- Treating internal implementation details as release-note content by default.

## Related

- B-0073
- B-0121
- B-0180
