---
schema: tanren.behavior.v0
id: B-0123
title: Recover from merge conflicts after parallel work lands
area: review-merge
personas: [solo-builder, team-builder, operator]
runtime_actors: [agent-worker]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user or Tanren worker can recover when parallel work causes a merge conflict so completed work is routed back through controlled remediation.

## Preconditions

- A merge or rebase conflict is detected for Tanren-managed work.
- The user has visibility of the affected work.

## Observable outcomes

- The conflict is visible on the affected spec or graph node.
- Tanren routes conflict resolution into follow-up work or human escalation.
- The original source signals and branch history remain traceable.

## Out of scope

- Silently discarding one side of the conflict.
- Resolving conflicts without source signals.

## Related

- B-0113
- B-0121
- B-0124
