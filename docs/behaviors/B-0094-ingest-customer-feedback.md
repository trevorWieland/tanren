---
schema: tanren.behavior.v0
id: B-0094
title: Capture human-authored product signals as candidate work
area: intake
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: [B-0095]
---

## Intent

A `solo-builder` or `team-builder` can capture human-authored product
signals — such as customer feedback, meeting notes, or stakeholder requests —
as candidate work so product input is not lost before prioritization.

## Preconditions

- An active project is selected.
- The user has permission to add intake items.

## Observable outcomes

- Each captured signal is recorded with its source, context, and originating
  user.
- Captured signals can be linked to existing specs, turned into new candidate
  specs, or held as background context for later prioritization.
- The user can defer or dismiss a signal without deleting it from the
  project's intake history.

## Out of scope

- Customer relationship management.
- Recording meetings or transcribing audio.
- Automatically promising delivery to the originator.
- Accepting generated candidate work without user review.

## Related

- B-0018
- B-0092
- B-0093
- B-0098
