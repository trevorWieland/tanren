---
schema: tanren.behavior.v0
id: B-0092
title: Maintain a product roadmap as planning context
area: product-planning
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can maintain a product roadmap as planning
context so product direction is visible before it becomes shaped specs.

## Preconditions

- A project exists.
- The user has permission to edit product planning context.

## Observable outcomes

- The roadmap records product priorities, phases, and open planning items at a
  user-visible level.
- Roadmap items can remain unshaped until the team is ready to turn them into
  specs.
- Roadmap changes remain traceable to the user who made them and can be
  reviewed through planning-change behaviors when policy requires it.

## Out of scope

- Automatically implementing roadmap items.
- Detailed prioritization, sequencing, or deferral decisions.
- Replacing product judgment with generated plans.

## Related

- B-0079
- B-0093
- B-0158
- B-0159
- B-0160
- B-0161
- B-0189
- B-0190
