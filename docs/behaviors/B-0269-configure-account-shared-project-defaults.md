---
schema: tanren.behavior.v0
id: B-0269
title: Configure shared project defaults for an account
area: configuration
personas: [solo-builder, team-builder, operator]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can configure shared project defaults for an account so repeated
multi-project setup works for personal and organizational use without requiring
each project to be configured from scratch.

## Preconditions

- The user has permission to manage shared defaults for the active account.
- The account can own or reach more than one project.

## Observable outcomes

- The user can define defaults for new project setup, runtime preferences,
  provider mappings, and optional integrations.
- Each project can show which defaults it inherited and which settings are
  project-specific.
- Shared defaults apply only to subsequent setup or work unless the user applies
  them to existing projects deliberately.
- Changes are attributed and visible in account or organization history.

## Out of scope

- Organization member roles, permissions, or team tracking.
- Storing secret values directly in defaults.
- Forcing all projects to share one identical configuration.

## Related

- B-0050
- B-0085
- B-0139
- B-0148
