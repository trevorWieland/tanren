---
id: B-0146
title: Detect build, test, lint, and release commands
area: repo-understanding
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can detect project commands so verification and release work can use the repository's real workflows.

## Preconditions

- An active project is connected to a repository.
- The user has permission to inspect repository configuration and scripts.

## Observable outcomes

- Tanren identifies likely build, test, lint, format, and release commands.
- Detected commands include confidence or evidence for why they were chosen.
- Ambiguous or missing commands are surfaced for user confirmation.

## Out of scope

- Running commands without the user's configured execution policy.
- Inventing commands when repository evidence is absent.

## Related

- B-0087
- B-0145
- B-0148
