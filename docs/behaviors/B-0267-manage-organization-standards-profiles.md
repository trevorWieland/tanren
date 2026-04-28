---
id: B-0267
title: Manage organization standards profiles
area: governance
personas: [team-builder, operator]
interfaces: [cli, api, mcp, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user with standards policy permission can manage organization standards
profiles so shared quality expectations can be versioned and reused across
projects.

## Preconditions

- The active organization exists.
- The user has permission to manage organization standards profiles.

## Observable outcomes

- The user can create, update, retire, and view organization standards profiles.
- Each profile records its intended scope, version, owner, and rationale.
- Profile changes are attributed and visible in organization history.
- Projects can later adopt or be required to use a profile through policy.

## Out of scope

- Editing a project's local standards directly.
- Applying a profile to a project without policy or project configuration.
- Treating standards profiles as secret material.

## Related

- B-0084
- B-0150
- B-0151
- B-0268
