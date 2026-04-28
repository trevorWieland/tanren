---
id: B-0206
title: See what is blocked and why
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see what work is blocked and why so delivery risks are visible before inspecting individual specs.

## Preconditions

- The user has visibility into a project, milestone, initiative, organization, or account scope.
- Visible work includes blocked or waiting items.

## Observable outcomes

- Blocked work is grouped by visible blocker type such as dependency, policy, approval, credentials, runtime, review, or human input.
- Each blocker links to the affected work and available next action.
- Hidden blockers are represented without leaking hidden details.

## Out of scope

- Resolving blockers automatically.
- Showing private details outside the user's visible scope.

## Related

- B-0017
- B-0111
- B-0187
