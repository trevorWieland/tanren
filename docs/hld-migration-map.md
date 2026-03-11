# HLD Migration Map

Purpose: prove that `tanren-hld.md` content was redistributed into canonical
repo docs before deletion.

| HLD Section | New Canonical Location |
|---|---|
| What Tanren Is | `docs/architecture/overview.md#what-tanren-is` |
| Why Tanren / raw agents / plan mode / cloud platforms | `docs/architecture/overview.md` + `README.md#what-tanren-is-not` |
| Three-Layer Model | `README.md#the-three-layers` + `docs/architecture/overview.md#three-layer-model` |
| Coordinator / Tanren / Agent Runtime boundaries | `docs/architecture/overview.md#three-layer-model` |
| Methodology System | `docs/methodology/system.md` |
| Command files inventory | `README.md#commands` + `docs/methodology/system.md#command-files` |
| Standards profiles | `README.md#profiles` + `docs/methodology/system.md#standards-profiles` |
| Product templates | `docs/methodology/system.md#product-templates` |
| Dual nature in practice | `docs/architecture/overview.md#what-tanren-is` |
| Bootstrapping a project | `README.md#quick-start` + `docs/getting-started/bootstrap.md` |
| One-off bootstrap commands | `docs/getting-started/bootstrap.md#2-one-time-knowledge-bootstrap` |
| Setup tanren execution (`tanren.yml`, `remote.yml`, `roles.yml`) | `docs/getting-started/bootstrap.md#3-configure-execution` |
| Opinionated core state machine | `docs/workflow/spec-lifecycle.md` |
| Issue intake and backlog curation | `docs/workflow/spec-lifecycle.md#ten-workflow-responsibilities` |
| Shape spec / orchestrate / walk spec / gate / feedback | `docs/workflow/spec-lifecycle.md` |
| Merge conflict resolution | `docs/workflow/spec-lifecycle.md#ten-workflow-responsibilities` |
| Dependency management | `docs/workflow/spec-lifecycle.md#ten-workflow-responsibilities` |
| Scope creep control | `docs/workflow/spec-lifecycle.md#orchestration-loop` |
| Spec lifecycle states | `docs/workflow/spec-lifecycle.md#core-lifecycle` |
| run-demo computer-use expectations | `docs/workflow/spec-lifecycle.md#run-demo-expectations` |
| Agent roles and role separation rationale | `README.md#agent-roles` + `docs/methodology/system.md#role-separation` |
| Execution environment lifecycle | `README.md#execution-environments` + `worker-manager/README.md#execution-environments` |
| Sub-adapter decomposition | `worker-manager/ADAPTERS.md` |
| Agent-proof remote design | `worker-manager/ADAPTERS.md` |
| Environment profiles and debug handoff | `worker-manager/README.md#remote-execution-setup` + `worker-manager/README.md#vm-management` |
| Pluggable integrations overview | `README.md#opinionated-core-vs-pluggable-integrations` |
| Current/planned adapters | `worker-manager/ADAPTERS.md` + `docs/roadmap.md` |
| Secret scoping (developer/project/infrastructure) | `README.md#configuration-scopes` + `docs/operations/security-secrets.md#secret-scopes` |
| Configuration scopes | `README.md#configuration-scopes` + `docs/operations/security-secrets.md#configuration-scopes` |
| Interaction methodologies (CLI/library/IPC/future HTTP) | `docs/interfaces.md` + `protocol/README.md` |
| Metering and observability | `worker-manager/README.md#event-system` + `docs/operations/observability.md` |
| Security model | `docs/operations/security-secrets.md#security-controls` |
| Roadmap (completed/near/medium/long) | `docs/roadmap.md` |
| Design principles | `docs/design-principles.md` |
