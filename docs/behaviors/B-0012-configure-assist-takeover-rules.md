---
schema: tanren.behavior.v0
id: B-0012
title: Configure when teammates can assist or take over each other's loops
area: team-coordination
personas: [team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can configure the rules that govern when another `team-builder` may
assist or take over an in-flight loop, so that each team can choose between
permissive ad-hoc collaboration and stricter pre-granted-consent workflows.

## Preconditions

- An active project is selected.
- The user has permission to change team coordination rules for the project.
  In organizational contexts this permission may be restricted by organization
  policy.

## Observable outcomes

- The user can configure, per project, whether assisting a teammate's loop
  (B-0010) is allowed ad-hoc for every project member or requires an
  explicit permission grant via B-0031.
- The user can configure, per project, whether taking over a teammate's
  loop (B-0011) is allowed ad-hoc for every project member or requires an
  explicit permission grant via B-0031.
- The current rule set is visible to every `team-builder` on the project.

## Out of scope

- Pairwise grants (e.g. "Alice may assist only Bob") — the permission
  model uses project-wide permissions only. To scope access to specific
  people, use explicit permission grants on the project (B-0031).
- Organization-wide default policies that constrain what a team can
  choose (covered by B-0040).
- Rules that depend on loop state (e.g. "takeover only allowed after N
  minutes of inactivity").

## Related

- B-0010
- B-0011
