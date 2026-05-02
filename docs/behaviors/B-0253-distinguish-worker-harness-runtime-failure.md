---
schema: tanren.behavior.v0
id: B-0253
title: Distinguish worker, harness, provider, and runtime failures
area: runtime-actor-contract
personas: [solo-builder, team-builder, observer, operator, integration-client]
runtime_actors: [agent-worker]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

An `agent-worker` can report failure categories distinctly so users can tell whether the worker, harness, provider, environment, or policy boundary failed.

## Preconditions

- Assigned work encounters a failure.

## Observable outcomes

- Failure source signals uses Tanren-level categories before provider-specific details.
- Retryability, required human action, and affected work are clear where known.
- Provider-specific details remain supporting source references and do not leak secrets.

## Out of scope

- Collapsing every failure into a generic worker error.
- Claiming root cause certainty when source signals are incomplete.

## Related

- B-0106
- B-0226
- B-0248
