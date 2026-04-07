# Documentation Index

This directory is the canonical location for tanren deep-dive documentation.
The root `README.md` stays concise and links here for detailed architecture,
workflow, operations, and roadmap material.

## Sections

- `architecture/overview.md` - product philosophy, three-layer model, and pluggability boundaries.
- `methodology/system.md` - command files, standards profiles, templates, and role usage.
- `workflow/spec-lifecycle.md` - opinionated lifecycle, orchestration loop, and scope controls.
- `getting-started/bootstrap.md` - installation and first-run bootstrap sequence.
- `operations/security-secrets.md` - secret tiers, configuration scopes, and security controls.
- `operations/observability.md` - event model, metrics, and operational queries.
- `interfaces.md` - CLI, library, and store interaction surfaces.
- `design-principles.md` - core design rules used for architectural decisions.
- `roadmap.md` - date-stamped delivery status and priorities.
- `hld-migration-map.md` - source-to-destination mapping for the retired HLD.
- `worker-README.md` - worker architecture and operations reference.
- `ADAPTERS.md` - adapter architecture, decomposition, and extension points.
- `rewrite/README.md` - clean-room rewrite planning docs (motivations, HLD, roadmap, principles).
  Includes container/execution-system planning and Rust stack recommendations.
  Also includes proposed crate/workspace orchestration guidance.

## Source of Truth Policy

- Each topic has one canonical page.
- Other docs should summarize and link, not duplicate full definitions.
- Behavioral or interface changes must update the canonical page in the same PR.
