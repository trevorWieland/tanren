---
schema: tanren.behavior.v0
id: B-0279
title: Route proactive analysis into planning
area: proactive-analysis
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can route proactive analysis into planning so automated findings become reviewable product-method inputs rather than ungoverned work.

## Preconditions

- A scheduled or manual analysis has produced findings, risks, or recommendations.
- The user has visibility into the affected planning scope.

## Observable outcomes

- Analysis results are recorded with scope, source, rationale, and affected behavior or planning context when known.
- The user can route results to behavior change, roadmap revision, spec candidate, deferred follow-up, or no action with rationale.
- Automated analysis does not directly bypass behavior canon, roadmap ordering, or required human approval.

## Out of scope

- Automatically accepting every analysis result as product work.
- Running the analysis itself.
- Replacing human product judgment where direction is uncertain.

## Related

- B-0096
- B-0097
- B-0150
- B-0161
- B-0189
- B-0190
- B-0286
- B-0287
