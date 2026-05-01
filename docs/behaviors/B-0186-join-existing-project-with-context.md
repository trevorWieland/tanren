---
schema: tanren.behavior.v0
id: B-0186
title: Join an existing project with current context
area: project-setup
personas: [team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can join an existing project with current context so they can contribute without reconstructing project history from outside Tanren.

## Preconditions

- The user has been granted access to the project.
- The project already has product, planning, execution, or standards context.

## Observable outcomes

- Tanren shows the project mission, roadmap, active work, relevant standards, and open attention items.
- The user can distinguish accepted shared context from drafts, proposals, and historical decisions.
- Missing onboarding context is visible rather than silently assumed.

## Out of scope

- Granting project access.
- Treating every visible project detail as actionable by the user.

## Related

- B-0027
- B-0089
- B-0200
