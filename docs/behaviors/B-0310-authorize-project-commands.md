---
schema: tanren.behavior.v0
id: B-0310
title: Authorize project commands from authenticated actor context
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

Project registration commands (connect an existing repository, create a new
project) evaluate a typed authorization decision after the caller is
authenticated and before any provider or store side effects run. The actor
context is derived from authenticated credentials, not from caller-supplied
parameters.

## Preconditions

- The caller is authenticated (API session cookie, MCP API key + configured
  capability context).

## Observable outcomes

- Personal-scope project registration (`org: None`) is allowed for any
  authenticated account.
- Organization-scope registration (`org: Some(id)`) is allowed only when the
  actor's organization matches the requested org.
- A scope mismatch or missing org membership produces an `access_denied`
  failure from the shared project failure taxonomy.
- The denial reason is a stable taxonomy code — no internal policy state
  leaks through the error surface.

## Out of scope

- Full role-based or permission-grant-based authorization (M-0004).
- Approval workflows or budget/quota policy.
- Actor resolution for CLI, TUI, or web surfaces beyond the API and MCP
  interfaces covered here.

## Related

- B-0025
- B-0026
- B-0239
