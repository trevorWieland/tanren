---
id: B-0012
title: Configure when teammates can assist or take over each other's loops
personas: [team-dev]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `team-dev` can configure the rules that govern when another `team-dev` may
assist or take over an in-flight loop, so that each team can choose between
permissive ad-hoc collaboration and stricter pre-granted-consent workflows.

## Preconditions

- An active project is selected.
- The user has permission to change team coordination rules for the project.
  In organizational contexts this permission may be restricted by organization
  policy.

## Observable outcomes

- The user can configure, per project, whether assisting a teammate's loop
  (B-0010) requires pre-granted consent or is allowed ad-hoc.
- The user can configure, per project, whether taking over a teammate's loop
  (B-0011) requires pre-granted consent or is allowed ad-hoc.
- The user can grant or revoke specific pairwise permissions (e.g. "Alice may
  assist Bob's loops") when pre-granted mode is in effect.
- The current rule set is visible to every `team-dev` on the project.

## Out of scope

- Organization-wide default policies that constrain what a team can choose
  (covered in a later governance area).
- Rules that depend on loop state (e.g. "takeover only allowed after N
  minutes of inactivity").

## Related

- B-0010
- B-0011
