---
schema: tanren.behavior.v0
id: B-0157
title: Explain why a spec is not ready
area: spec-quality
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see why a spec is not ready so unresolved shaping, policy, dependency, or quality concerns are actionable.

## Preconditions

- A spec exists and is not ready.
- The user has visibility into the spec.

## Observable outcomes

- The not-ready explanation names user-visible blockers or concerns.
- Each concern links to the relevant product context, dependency, policy, or missing source signals where visible.
- Hidden details are redacted without hiding that a blocker exists.

## Out of scope

- Granting access to hidden project or organization details.
- Explaining internal scheduler or storage implementation.

## Related

- B-0017
- B-0021
- B-0156
