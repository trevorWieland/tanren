---
id: B-0246
title: Submit evidence artifacts for assigned work
area: runtime-actor-contract
personas: [solo-builder, team-builder, observer, operator]
runtime_actors: [agent-worker]
interfaces: [api, mcp, daemon]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can submit evidence artifacts for assigned work so execution results remain reviewable and linked to product context.

## Preconditions

- The worker has an active assignment with evidence obligations.
- The evidence is within the assignment's allowed scope.

## Observable outcomes

- Submitted evidence links to the assigned work, phase or intent, runtime attempt, and source actor.
- Evidence can include plans, patches, checks, findings, demos, logs, or outcome summaries as appropriate to the assignment.
- Evidence that cannot be accepted is rejected with an actionable reason.

## Out of scope

- Accepting evidence for unrelated work.
- Treating unsupported artifact shapes as valid because a worker produced them.

## Related

- B-0116
- B-0169
- B-0243
