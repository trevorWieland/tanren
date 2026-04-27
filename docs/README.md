# Documentation Index

This directory is the canonical location for Tanren documentation. The root
`README.md` stays concise and links here for detailed architecture, roadmap,
methodology, and operations material.

## Sections

- `roadmap/README.md` - product roadmap suite index.
- `roadmap/ROADMAP.md` - phased product roadmap.
- `behaviors/README.md` - product behavior catalog.
- `../tests/bdd/README.md` - executable behavior evidence rules.
- `architecture/overview.md` - system architecture and boundaries.
- `architecture/install-targets.md` - install render targets, merge policy, and MCP env contract.
- `architecture/agent-tool-surface.md` - typed tool surface and canonical CLI fallback shape.
- `architecture/evidence-schemas.md` - generated evidence artifact contracts.
- `methodology/commands-install.md` - canonical install/runtime contract.
- `methodology/system.md` - command files, standards profiles, and role usage.
- `workflow/spec-lifecycle.md` - lifecycle, orchestration loop, and scope controls.
- `operations/security-secrets.md` - secret tiers, configuration scopes, and security controls.
- `design-principles.md` - core design rules used for architecture decisions.
- `roadmap.md` - date-stamped delivery status and priorities.
- `ADAPTERS.md` - adapter architecture, decomposition, and extension points.

## Source of Truth Policy

- Each topic has one canonical page.
- Other docs should summarize and link, not duplicate full definitions.
- Behavior, interface, lifecycle, or security changes must update the canonical
  page in the same PR.
