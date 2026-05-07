---
schema: tanren.behavior.v0
id: B-0294
title: Reject experience drift from the design system
area: experience-design
personas: [solo-builder, team-builder]
surfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can rely on Tanren to reject or flag
experience work that references unknown design patterns or invents local
experience rules where a registered design-system pattern applies, so agents
do not drift away from the central design canon.

## Preconditions

- A design-system pattern registry exists.
- The user or an agent is editing roadmap nodes, experience contracts, proof
  artifacts, or surface implementation plans.

## Observable outcomes

- A roadmap node whose `design_pattern_refs` includes an unknown pattern ID is
  rejected with a message that names the offending node and the registry that
  defines the allowed pattern IDs.
- An expected-evidence record whose `design_pattern_refs` includes an unknown
  pattern ID is rejected with a message that names the behavior and node.
- Removing or renaming a design pattern that roadmap nodes or experience
  contracts still reference is reported as drift instead of silently dropping
  the obligation.
- Work that deliberately departs from a registered pattern records an explicit
  deviation and rationale before implementation or walk acceptance.

## Out of scope

- Defining design-system records. Use `define-design-system`.
- Defining project surfaces. Use `define-surfaces`.
- Producing the behavior proof itself. That is per-node `expected_evidence`.

## Related

- B-0290
- B-0292
- B-0293
