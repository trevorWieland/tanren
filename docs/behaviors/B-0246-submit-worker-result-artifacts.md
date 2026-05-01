---
schema: tanren.behavior.v0
id: B-0246
title: Submit worker result artifacts for assigned work
area: runtime-actor-contract
personas: [solo-builder, team-builder, observer, operator]
runtime_actors: [agent-worker]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can submit worker result artifacts for assigned work so execution results remain reviewable and linked to product context.

## Preconditions

- The worker has an active assignment with proof obligations.
- The submitted result artifacts are within the assignment's allowed scope.

## Observable outcomes

- Submitted source references link to the assigned work, phase or intent, runtime attempt, and source actor.
- Source signals can include plans, patches, checks, findings, demos, logs, or outcome summaries as appropriate to the assignment.
- Source signals that cannot be accepted are rejected with an actionable reason.

## Out of scope

- Accepting source signals for unrelated work.
- Treating unsupported artifact shapes as valid because a worker produced them.

## Related

- B-0116
- B-0169
- B-0243
