---
schema: tanren.behavior.v0
id: B-0173
title: Surface stale assumptions when context changes
area: decision-memory
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see stale assumptions when context changes so Tanren does not keep planning against outdated facts.

## Preconditions

- Product, repository, roadmap, feedback, or execution source signals have changed.
- Recorded assumptions or decisions depend on the changed context.

## Observable outcomes

- Tanren identifies assumptions that may need review.
- The user can see what changed and which work may be affected.
- Reviewed assumptions can be reaffirmed, revised, or retired.

## Out of scope

- Automatically rewriting product direction.
- Flagging every old decision without a meaningful changed context.

## Related

- B-0098
- B-0161
- B-0170
