---
schema: tanren.behavior.v0
id: B-0286
title: Schedule or run proactive analysis
area: proactive-analysis
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can schedule or run proactive analysis so risks, regressions, and
improvement opportunities can be found outside a single spec request.

## Preconditions

- A project exists.
- The user has permission to run or schedule analysis for the selected scope.
- The selected analysis type is available for the project.

## Observable outcomes

- The user can start or schedule supported analysis such as standards sweeps,
  security review, dependency review, mutation testing, performance profiling,
  benchmarks, or post-ship health checks.
- Analysis runs show scope, source, timing, status, and non-secret results.
- Results are available for review and routing through planning.

## Out of scope

- Automatically accepting discovered work into the roadmap.
- Running analysis without respecting configured execution and credential
  boundaries.
- Treating every analysis finding as product scope.

## Related

- B-0096
- B-0230
- B-0279
- B-0287
