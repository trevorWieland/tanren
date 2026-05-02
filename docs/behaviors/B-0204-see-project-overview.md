---
schema: tanren.behavior.v0
id: B-0204
title: See a project overview
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see a project overview so current progress, risk, health, and attention needs are understandable at a glance.

## Preconditions

- The user has visibility into the project.

## Observable outcomes

- The overview summarizes mission, roadmap progress, active work, blockers, quality, health, and recent outcomes.
- Each summary links to supporting detail or source signals where visible.
- Hidden or unavailable information is marked rather than treated as healthy.

## Out of scope

- Replacing detailed project views.
- Granting authority to act on every visible item.

## Related

- B-0027
- B-0032
- B-0209
