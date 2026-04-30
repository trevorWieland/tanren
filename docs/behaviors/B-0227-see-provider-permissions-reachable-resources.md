---
schema: tanren.behavior.v0
id: B-0227
title: See provider permissions and reachable resources
area: integration-management
personas: [team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see provider permissions and reachable resources so Tanren's external reach is understandable.

## Preconditions

- A provider connection exists.
- The user has visibility into provider connection metadata.

## Observable outcomes

- Tanren shows provider-level capabilities, resource scopes, and known limitations.
- Overly broad, missing, or ambiguous permissions can be flagged for review.
- Secret values and hidden provider resources remain hidden.

## Out of scope

- Granting additional provider permissions.
- Replacing provider-native access controls.

## Related

- B-0104
- B-0225
- B-0231
