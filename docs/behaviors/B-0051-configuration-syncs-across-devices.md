---
schema: tanren.behavior.v0
id: B-0051
title: Configuration follows the account across devices
area: configuration
personas: [solo-builder, team-builder, observer, operator]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can sign in to the same account on a different device — phone,
low-power laptop, full laptop — and find their configuration already in
effect, so that they can pick up work without reconfiguring each device.

## Preconditions

- The user is signed into an account on more than one device.

## Observable outcomes

- User-tier configuration and credentials (B-0048) follow the account to
  every device the user signs into.
- Project-tier configuration (B-0049) and organization-tier configuration
  (B-0050) are available on every device a user accesses the project or
  organization from.
- Configuration changes made on one device become available on the user's
  other signed-in devices without manual re-entry.
- A device the user signs out of or removes no longer receives
  configuration for that account.

## Out of scope

- Device-specific configuration profiles (e.g. different settings on the
  phone vs the laptop for the same account).
- Offline editing and later conflict resolution across devices.

## Related

- B-0048
- B-0049
- B-0050
