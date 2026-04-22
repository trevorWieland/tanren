# Documentation Index

This directory is the canonical location for tanren deep-dive documentation.
The root `README.md` stays concise and links here for detailed architecture,
workflow, operations, and roadmap material.

## Sections

- `methodology/commands-install.md` - canonical install/runtime contract (`tanren-cli`, `tanren-mcp`).
- `architecture/install-targets.md` - install render targets, merge policy, and MCP env contract.
- `architecture/agent-tool-surface.md` - typed tool surface and canonical CLI fallback shape.
- `rewrite/PHASE0_PROOF_RUNBOOK.md` - Phase 0 proof run/verify acceptance flow.
- `rewrite/PHASE1_PROOF_BDD.md` - Phase 1 behavioral invariants and witnesses.
- `architecture/overview.md` - product philosophy, three-layer model, and pluggability boundaries.
- `methodology/system.md` - command files, standards profiles, templates, and role usage.
- `workflow/spec-lifecycle.md` - lifecycle, orchestration loop, and scope controls.
- `operations/security-secrets.md` - secret tiers, configuration scopes, and security controls.
- `operations/observability.md` - event model, metrics, and operational queries.
- `interfaces.md` - CLI, library, and store interaction surfaces.
- `design-principles.md` - core design rules used for architectural decisions.
- `roadmap.md` - date-stamped delivery status and priorities.
- `hld-migration-map.md` - source-to-destination mapping for the retired HLD.
- `worker-README.md` - worker architecture and operations reference.
- `ADAPTERS.md` - adapter architecture, decomposition, and extension points.
- `getting-started/bootstrap.md` - legacy Python-era bootstrap notes (archived; not Phase 0 acceptance path).
- `rewrite/README.md` - clean-room rewrite planning docs and history.

## Source of Truth Policy

- Each topic has one canonical page.
- Other docs should summarize and link, not duplicate full definitions.
- Behavioral or interface changes must update the canonical page in the same PR.
