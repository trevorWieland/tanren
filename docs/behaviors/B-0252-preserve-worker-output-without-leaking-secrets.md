---
schema: tanren.behavior.v0
id: B-0252
title: Preserve worker output without leaking secrets
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

An `agent-worker` can preserve output needed for audit and recovery without leaking secrets into routine views.

## Preconditions

- A worker assignment produces logs, messages, artifacts, or provider output.
- The output may contain sensitive material or hidden scope details.

## Observable outcomes

- Useful output remains linked to the assignment and source-reference trail.
- Secret values and hidden details are redacted or withheld according to policy.
- Redaction is visible so users know source signals were limited.

## Out of scope

- Discarding all output because some output is sensitive.
- Treating redaction as proof that no problem occurred.

## Related

- B-0127
- B-0169
- B-0246
