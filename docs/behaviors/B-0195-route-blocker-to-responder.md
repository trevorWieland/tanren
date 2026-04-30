---
schema: tanren.behavior.v0
id: B-0195
title: Route a blocker to an appropriate responder
area: team-coordination
personas: [team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can route a blocker to an appropriate responder so paused work reaches someone able to resolve it.

## Preconditions

- A visible work item is blocked or waiting for input.
- Tanren has enough context to identify candidate responders or required permissions.

## Observable outcomes

- The blocker identifies why input is needed and what kind of response can unblock it.
- Tanren can route the blocker to a person, group, or permission-holder without exposing hidden details.
- The routing decision and response history remain visible from the blocked work.

## Out of scope

- Assuming a fixed role owns every blocker type.
- Revealing private user or organization details through routing.

## Related

- B-0005
- B-0187
- B-0194
