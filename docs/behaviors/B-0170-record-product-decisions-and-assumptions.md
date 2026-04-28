---
id: B-0170
title: Record product decisions and assumptions
area: decision-memory
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can record product decisions and assumptions so future planning understands why work was shaped a certain way.

## Preconditions

- An active project exists.
- The user has permission to add product planning context.

## Observable outcomes

- Decisions and assumptions are recorded with source, time, and attribution.
- Related roadmap items, specs, standards, or evidence can link to them.
- Later changes can supersede an assumption without deleting its history.

## Out of scope

- Treating assumptions as proven facts.
- Requiring all decisions to come from a single interface.

## Related

- B-0098
- B-0171
- B-0173
