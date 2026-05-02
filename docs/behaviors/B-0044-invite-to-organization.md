---
schema: tanren.behavior.v0
id: B-0044
title: Invite a person to an organization
area: governance
personas: [team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `team-builder` with the required permission can invite a person into an
organization, so that new members can be brought in either by creating a
fresh account through the invitation or by attaching an existing standalone
account to the organization.

## Preconditions

- The user has permission to invite members into the organization.
- The context is organizational; this behavior does not apply to personal
  projects.

## Observable outcomes

- The user can send an invitation addressed to a specific person, with
  the organization-level permissions or role the invitee will hold on
  acceptance.
- The invitation is visible on the inviting user's side and can be revoked
  before it is accepted.
- An invitation can be accepted by creating a new account (B-0043) or by
  attaching an existing account (B-0045).
- After acceptance, the invitee is a member of the organization with the
  organization-level permissions specified in the invitation. Project
  access within the organization is granted separately via B-0031, not by
  the invitation itself.

## Out of scope

- Bulk invitations or uploading lists of invitees.
- Automatic provisioning based on directory or identity-provider groups.

## Related

- B-0043
- B-0045
- B-0031
