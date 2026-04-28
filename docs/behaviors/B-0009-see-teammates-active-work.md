---
id: B-0009
title: See teammates' active work alongside my own
area: team-coordination
personas: [team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can see a default view of Tanren that highlights their own active
loops while also showing their teammates' active loops alongside them, so that
they have a clear picture of who is working on what without having to switch
views.

## Preconditions

- The user is part of a team that has more than one Tanren user on a shared
  project.
- The user has visibility scope over their teammates' work.

## Observable outcomes

- The user's own active loops are visually prominent in the default view.
- Teammates' active loops are visible alongside the user's own but clearly
  de-emphasized, so the user can tell at a glance which loops are theirs.
- The user can drill from this view into any teammate's loop they have
  visibility of (see B-0003, B-0008).
- Loops belonging to teammates the user has no visibility of are not shown.

## Out of scope

- Cross-team or organization-wide views (covered separately in the
  observer-facing area).
- Custom filters, groupings, or saved views beyond the default layout.

## Related

- B-0003
- B-0008
