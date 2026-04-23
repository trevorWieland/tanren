---
id: B-0042
title: See the history of permission changes
personas: [team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` or `observer` can see a chronological record of permission
changes within a project or organization they have visibility of, so that
changes to who can do what are attributable and auditable.

## Preconditions

- Has visibility scope over the project or organization.

## Observable outcomes

- The user can see an ordered list of permission changes, each attributed
  to the user who made the change and stamped with when it happened.
- Entries cover grants, revocations, role applications, role changes, and
  organization-policy edits.
- The history is available for both active and past members of the project
  or organization.

## Out of scope

- Editing or redacting history entries.
- Rolling a permission state back to an earlier point in the history.

## Related

- B-0014
- B-0031
- B-0038
- B-0040
