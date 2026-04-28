---
id: B-0147
title: Summarize architecture and major risk areas
area: repo-understanding
personas: [solo-builder, team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see a repository architecture and risk summary so planning starts with an understandable map of the codebase.

## Preconditions

- An active project is connected to a repository.
- The user has permission to inspect repository evidence.

## Observable outcomes

- Tanren identifies major components, boundaries, and dependencies at a user level.
- Risk areas are presented with evidence and uncertainty.
- The summary can be refreshed as the repository changes.

## Out of scope

- Replacing a detailed architecture review.
- Exposing repository details outside the user's visible scope.

## Related

- B-0096
- B-0145
- B-0173
