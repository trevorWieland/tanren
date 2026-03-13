# HLD Migration Map

Purpose: prove that `tanren-hld.md` content was redistributed into canonical
repo docs before deletion.

| HLD Section | New Canonical Location |
|---|---|
| What Tanren Is | [`architecture/overview.md#what-tanren-is`](architecture/overview.md#what-tanren-is) |
| Why Tanren / raw agents / plan mode / cloud platforms | [`architecture/overview.md`](architecture/overview.md) + [`../README.md#what-tanren-is-not`](../README.md#what-tanren-is-not) |
| Three-Layer Model | [`../README.md#the-three-layers`](../README.md#the-three-layers) + [`architecture/overview.md#three-layer-model`](architecture/overview.md#three-layer-model) |
| Coordinator / Tanren / Agent Runtime boundaries | [`architecture/overview.md#three-layer-model`](architecture/overview.md#three-layer-model) |
| Methodology System | [`methodology/system.md`](methodology/system.md) |
| Command files inventory | [`../README.md#commands`](../README.md#commands) + [`methodology/system.md#command-files`](methodology/system.md#command-files) |
| Standards profiles | [`../README.md#profiles`](../README.md#profiles) + [`methodology/system.md#standards-profiles`](methodology/system.md#standards-profiles) |
| Product templates | [`methodology/system.md#product-templates`](methodology/system.md#product-templates) |
| Dual nature in practice | [`architecture/overview.md#what-tanren-is`](architecture/overview.md#what-tanren-is) |
| Bootstrapping a project | [`../README.md#quick-start`](../README.md#quick-start) + [`getting-started/bootstrap.md`](getting-started/bootstrap.md) |
| One-off bootstrap commands | [`getting-started/bootstrap.md#2-one-time-knowledge-bootstrap`](getting-started/bootstrap.md#2-one-time-knowledge-bootstrap) |
| Setup tanren execution (`tanren.yml`, `remote.yml`, `roles.yml`) | [`getting-started/bootstrap.md#3-configure-execution`](getting-started/bootstrap.md#3-configure-execution) |
| Opinionated core state machine | [`workflow/spec-lifecycle.md`](workflow/spec-lifecycle.md) |
| Issue intake and backlog curation | [`workflow/spec-lifecycle.md#ten-workflow-responsibilities`](workflow/spec-lifecycle.md#ten-workflow-responsibilities) |
| Shape spec / orchestrate / walk spec / gate / feedback | [`workflow/spec-lifecycle.md`](workflow/spec-lifecycle.md) |
| Merge conflict resolution | [`workflow/spec-lifecycle.md#ten-workflow-responsibilities`](workflow/spec-lifecycle.md#ten-workflow-responsibilities) |
| Dependency management | [`workflow/spec-lifecycle.md#ten-workflow-responsibilities`](workflow/spec-lifecycle.md#ten-workflow-responsibilities) |
| Scope creep control | [`workflow/spec-lifecycle.md#orchestration-loop`](workflow/spec-lifecycle.md#orchestration-loop) |
| Spec lifecycle states | [`workflow/spec-lifecycle.md#core-lifecycle`](workflow/spec-lifecycle.md#core-lifecycle) |
| run-demo computer-use expectations | [`workflow/spec-lifecycle.md#run-demo-expectations`](workflow/spec-lifecycle.md#run-demo-expectations) |
| Agent roles and role separation rationale | [`methodology/system.md#agent-roles`](methodology/system.md#agent-roles) + [`methodology/system.md#role-separation`](methodology/system.md#role-separation) |
| Execution environment lifecycle | [`../README.md#execution-environments`](../README.md#execution-environments) |
| Sub-adapter decomposition | [`ADAPTERS.md`](ADAPTERS.md) |
| Agent-proof remote design | [`ADAPTERS.md`](ADAPTERS.md) |
| Environment profiles and debug handoff | [`ADAPTERS.md#sshexecutionenvironment`](ADAPTERS.md#sshexecutionenvironment) |
| Pluggable integrations overview | [`architecture/overview.md#opinionated-core-vs-pluggable-integrations`](architecture/overview.md#opinionated-core-vs-pluggable-integrations) |
| Current/planned adapters | [`ADAPTERS.md`](ADAPTERS.md) + [`roadmap.md`](roadmap.md) |
| Secret scoping (developer/project/infrastructure) | [`../README.md#configuration-scopes`](../README.md#configuration-scopes) + [`operations/security-secrets.md#secret-scopes`](operations/security-secrets.md#secret-scopes) |
| Configuration scopes | [`../README.md#configuration-scopes`](../README.md#configuration-scopes) + [`operations/security-secrets.md#configuration-scopes`](operations/security-secrets.md#configuration-scopes) |
| Interaction methodologies (CLI/library/IPC/future HTTP) | [`interfaces.md`](interfaces.md) + [`../protocol/README.md`](../protocol/README.md) |
| Metering and observability | [`ADAPTERS.md#eventemitter`](ADAPTERS.md#eventemitter) + [`operations/observability.md`](operations/observability.md) |
| Security model | [`operations/security-secrets.md#security-controls`](operations/security-secrets.md#security-controls) |
| Roadmap (completed/near/medium/long) | [`roadmap.md`](roadmap.md) |
| Design principles | [`design-principles.md`](design-principles.md) |
