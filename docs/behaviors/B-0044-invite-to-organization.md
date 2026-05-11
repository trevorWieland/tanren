---
schema: tanren.behavior.v0
id: B-0044
title: Invite a person to an organization
area: governance
personas: [team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [organizational]
product_status: accepted
verification_status: implementation-ready
supersedes: []
---

## Intent

A `team-builder` with the `invite` organization permission can invite a
person into an organization, so that new members can be brought in either
by creating a fresh account through the invitation or by attaching an
existing standalone account to the organization.

## Preconditions

- The caller is authenticated with a valid session.
- The caller holds the `invite` organization permission for the target
  organization.
- The context is organizational; this behavior does not apply to personal
  (non-organization) contexts. A `personal_context_not_allowed` failure is
  returned if the caller attempts an invitation operation outside an
  organizational scope.
- The recipient identifier is a valid opaque token (R-0006 token-based
  visibility).

## Observable outcomes

- The user can create an invitation addressed to a specific recipient
  identifier, with a selected set of organization-level permissions the
  invitee will hold on acceptance.
- The invitation response exposes: the invitation id, the recipient
  identifier, the org id, the selected permissions, the lifecycle status,
  proof and source links, and non-secret visibility fields.
- The user can list invitations for the organization, paginated by cursor.
- The user can look up a single invitation by id.
- The invitation is visible on the inviting user's side and can be revoked
  before it is accepted.
- Revoking a pending invitation transitions its status to `revoked`; an
  already-consumed invitation returns `invitation_already_consumed`.
- An invitation can be accepted by creating a new account (B-0043) or by
  attaching an existing account (B-0045).
- After acceptance, the invitee is a member of the organization with the
  organization-level permissions specified in the invitation. Project
  access within the organization is granted separately via B-0031, not by
  the invitation itself.

## Failure taxonomy

- `auth_required` — no valid session.
- `permission_denied` — caller lacks the `invite` permission.
- `validation_failed` — input failed contract-level validation.
- `invitation_not_found` — no matching invitation for the requested id.
- `invitation_already_consumed` — invitation already accepted or revoked.
- `personal_context_not_allowed` — operation attempted outside an
  organizational context.

## Out of scope

- Bulk invitations or uploading lists of invitees.
- Automatic provisioning based on directory or identity-provider groups.
- Existing-account acceptance flow (B-0045); R-0006 covers token-based
  recipient visibility only.

## Related

- B-0043
- B-0045
- B-0031
