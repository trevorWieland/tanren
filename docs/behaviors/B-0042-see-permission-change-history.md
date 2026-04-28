---
id: B-0042
title: See the change history for a project or organization
area: governance
personas: [team-builder, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` or `observer` can see a chronological record of changes
within a project or organization they have visibility of — permission
changes, access changes, and configuration changes — so that how the
project or organization runs is attributable and auditable over time.

## Preconditions

- Has visibility scope over the project or organization.

## Observable outcomes

- The user can see an ordered list of changes, each attributed to the
  user who made the change and stamped with when it happened.
- Entries cover permission grants and revocations, role applications and
  edits, access additions and removals, organization policy edits, and
  configuration changes at any tier the user has visibility of.
- The user can filter entries by change type so that a specific concern
  (for example, only access changes or only configuration changes) can be
  reviewed on its own.
- The history is available for current members and for past members.

## Out of scope

- Editing or redacting history entries.
- Rolling a project or organization back to an earlier state from the
  history.

## Related

- B-0014
- B-0031
- B-0038
- B-0040
- B-0049
- B-0050
