# Documentation Index

This directory is the canonical location for Tanren documentation. Start with
the root [README](../README.md) for the product-to-proof vision, then use this
index for the durable source pages.

Tanren is documented as a method, not only as code. A new reader should be able
to follow the full chain:

```text
product brief -> behaviors -> roadmap DAG -> shaped specs -> orchestration -> evidence -> feedback
```

Architecture docs explain the Rust control plane that enforces that chain.
Behavior docs explain the product outcomes Tanren is trying to make true.
Methodology docs explain how agent commands fit into the workflow.

## Sections

- `vision.md` - full product-to-proof vision and boundaries.
- `behaviors/README.md` - product behavior catalog.
- `roadmap/README.md` - roadmap DAG source-of-truth model.
- `roadmap/ROADMAP.md` - current human-readable roadmap view.
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
- `ADAPTERS.md` - adapter architecture, decomposition, and extension points.

## Source of Truth Policy

- Each topic has one canonical page.
- Other docs should summarize and link, not duplicate full definitions.
- Behavior, interface, lifecycle, or security changes must update the canonical
  page in the same PR.
