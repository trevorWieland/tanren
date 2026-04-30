---
schema: tanren.behavior.v0
id: B-0247
title: Report blockers with actionable options
area: runtime-actor-contract
personas: [solo-builder, team-builder, operator]
runtime_actors: [agent-worker]
interfaces: [api, mcp, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can report blockers with actionable options so paused execution can be routed to a useful human or policy decision.

## Preconditions

- The worker has an active assignment.
- The assignment cannot proceed without input, access, policy change, or external recovery.

## Observable outcomes

- The blocker records summary, affected work, blocker category, source signals, and proposed options.
- Users can see what response would unblock the work where they have visibility.
- Blockers can route to the configured responder, approval flow, or follow-up work.

## Out of scope

- Escalating every transient failure as a human blocker.
- Revealing hidden credentials or provider details in blocker text.

## Related

- B-0005
- B-0195
- B-0239
