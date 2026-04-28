---
id: B-0266
title: Run a stacked-diff dependent spec against an available base
area: planner-orchestration
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can run a dependent spec against an available
base from another spec so stacked work can proceed before the dependency has
merged to the primary branch.

## Preconditions

- The dependent spec declares the base spec as a dependency.
- The base spec has an available branch, artifact, or accepted evidence that
  project policy treats as usable.
- The user has visibility into the dependency relationship.

## Observable outcomes

- The dependent spec records which base branch or artifact it is building
  against.
- Tanren distinguishes a usable stacked dependency from an unavailable blocker.
- When the base changes or lands, the dependent spec shows whether rebase or
  conflict recovery is needed.
- Dependency and rebase decisions remain visible in the work history.

## Out of scope

- Treating every unfinished dependency as usable.
- Hiding unresolved dependency risk because a stacked base exists.
- Resolving merge or intent conflicts without evidence.

## Related

- B-0017
- B-0029
- B-0113
- B-0123
- B-0124
