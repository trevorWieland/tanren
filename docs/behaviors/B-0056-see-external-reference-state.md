---
id: B-0056
title: See the current state of external issues referenced by a spec
area: external-tracker
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder`, `team-builder`, or `observer` can see the current state of any
external issues referenced by a spec — the originating ticket and any
external issues listed as dependencies — so that they understand where the
external work stands without leaving Tanren.

## Preconditions

- The spec references at least one external ticket or external issue.
- The user has visibility of the spec.

## Observable outcomes

- For each external reference on a spec, the user can see the reference's
  current state as reported by the external tracker (for example, open,
  closed, in progress) along with a link to the tracker.
- External references whose state cannot be read — because the tracker is
  unreachable, not connected, or the user lacks authorization — are shown
  as unresolved with a clear indication of why.
- B-0017 honors external references the same way it honors spec
  dependencies when deciding whether a loop can start.

## Out of scope

- Modifying the external issue from within Tanren.
- Proactive alerts when an external reference changes state.

## Related

- B-0017
- B-0018
- B-0029
- B-0052
