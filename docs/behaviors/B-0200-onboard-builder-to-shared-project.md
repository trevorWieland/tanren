---
schema: tanren.behavior.v0
id: B-0200
title: Onboard a builder to shared project context
area: team-coordination
personas: [team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` can be onboarded to shared project context so they understand how to contribute before taking action.

## Preconditions

- The user has access to the project.
- The project has accepted context such as mission, roadmap, standards, active work, or decision history.

## Observable outcomes

- Tanren presents the project mission, current roadmap, standards, active work, open risks, and contribution expectations.
- The user can see which areas are recommended reading versus required before contributing.
- Onboarding progress is visible to the user without becoming a performance metric.

## Out of scope

- Replacing human mentorship or organization-specific onboarding.
- Granting permissions automatically because onboarding was viewed.

## Related

- B-0186
- B-0188
- B-0199
