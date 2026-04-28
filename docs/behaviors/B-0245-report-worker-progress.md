---
id: B-0245
title: Report worker progress durably
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

An `agent-worker` can report progress durably so users can see what active execution is doing without watching raw harness output.

## Preconditions

- The worker has an active assignment.
- The user has visibility into the assigned work or runtime state.

## Observable outcomes

- Progress reports identify current phase or intent, work item, high-level activity, and freshness.
- Progress remains available after interface refresh, reconnect, or worker restart.
- Progress reports avoid leaking secrets, raw tokens, or hidden provider details.

## Out of scope

- Streaming every low-level log line as product progress.
- Treating progress text as acceptance evidence by itself.

## Related

- B-0003
- B-0008
- B-0103
