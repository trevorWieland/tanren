---
schema: tanren.behavior.v0
id: B-0311
title: Resolve source-control providers through a registry abstraction
area: project-setup
personas: [solo-builder, team-builder]
interfaces: [api, mcp]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

Production API and MCP runtimes resolve source-control providers through a
typed registry abstraction rather than a hard-coded `None` sentinel. When no
provider is configured, project commands fail with `provider_not_configured`
before any external side effects are dispatched.

## Preconditions

- The caller is authenticated.
- A provider registry is wired into the runtime at startup.

## Observable outcomes

- When the registry holds a configured provider, project connect and create
  commands succeed normally.
- When the registry returns no provider (e.g. `NullProviderRegistry`), project
  connect and create commands fail with `provider_not_configured` (HTTP 503
  on the API surface, `ProjectFailureReason::ProviderNotConfigured` on MCP).
- The failure occurs before any provider or store side effects.
- Test-hook constructors can still inject a fixture provider for BDD.

## Out of scope

- Real external provider adapters (GitHub, GitLab).
- Credential storage or provider connection management.
- CLI, TUI, and web surfaces (they use their own provider wiring).

## Related

- B-0025
- B-0026
- B-0310
