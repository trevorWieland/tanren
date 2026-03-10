# tanren

Opinionated orchestration engine for agentic software development.

tanren defines *what work happens and in what order* — issue intake, spec shaping, autonomous execution, gate checks, feedback loops, and dependency management. It does not choose which model or CLI to use. That decision belongs to the agent runtime layer.

## The Three Layers

```
┌─────────────────────────────────────────────────────────┐
│                    COORDINATOR                          │
│  Identity, authorization, metering, developer interface │
│  (Web dashboard primary; CLI built-in; chat pluggable)  │
├─────────────────────────────────────────────────────────┤
│                      TANREN                             │
│  Issue intake, spec lifecycle, dependency DAG,          │
│  gate checks, feedback loops, environment provisioning  │
│  (Opinionated workflow, pluggable integrations)         │
├─────────────────────────────────────────────────────────┤
│                   AGENT RUNTIME                         │
│  Role→tool mapping, model routing, coding CLI           │
│  management, auth (API key or OAuth)                    │
│  (Developer/org-scoped configuration)                   │
└─────────────────────────────────────────────────────────┘
```

**The bright line: tanren decides what to do. The agent runtime decides how to do it.** Tanren never picks a model. The runtime never decides which spec to run next.

## Opinionated Core vs Pluggable Integrations

The opinionated core IS tanren — these workflows define the product:

1. **Issue Intake & Backlog Curation** — well-crafted, dependency-aware, independently-verifiable issues
2. **Shape Spec** — decompose an issue into a structured, actionable spec
3. **Orchestrate** — plan the execution: what changes, in what order
4. **Walk Spec** — execute the planned work inside an execution environment
5. **Gate Check** — validate: tests, lint, type checks, CI
6. **Handle Feedback** — process review comments, apply changes, re-validate
7. **Merge Conflict Resolution** — resolve conflicts using spec-level context
8. **Dependency Management** — blocking/blocked relationships, stacked diffs, rebase cascades
9. **Scope Creep Control** — shaped spec is the contract; drive-by fixes for small items, deferred issues for everything else
10. **Spec Lifecycle** — state machine: draft → shaped → executing → validating → review → merged

Integrations are pluggable via adapter interfaces:

| Category | Built-in | Also Supports |
|---|---|---|
| Issue Source | GitHub Issues | Linear, Jira, custom |
| Source Control | GitHub | GitHub Enterprise, GitLab, Bitbucket |
| Execution Environment | Local subprocess | Docker, remote VM via SSH |
| CI/CD | GitHub Actions | GitLab CI, Jenkins, CircleCI |
| Secret Management | Flat file (~/.tanren/secrets.env) | Vault, AWS/GCP Secret Manager |
| Event/Metrics Storage | SQLite | Postgres, BigQuery, custom |
| Token Usage Collection | Log parsing | Metering proxy |
| Coordinator Interface | Web dashboard + CLI | Discord, Slack, Teams |

## Agent Roles

Tanren's workflow involves distinct agent roles, each fulfilled by a different CLI + model combination. The mapping from role to tool lives in the agent runtime config, not in tanren.

- **conversation** — talks to the developer (shaping specs, clarifying requirements). Needs strong reasoning.
- **implementation** — writes code inside the execution environment (walking the spec). Needs strong coding ability.
- **audit** — reviews specs for completeness, validates implementation against spec. Needs careful analytical reasoning.
- **feedback** — processes review comments and applies changes. Needs to understand human intent from terse PR comments.
- **conflict-resolution** — resolves merge conflicts using spec-level context. Needs to understand the intent of two divergent changes.

## Quick Start — Project Install

```bash
cd your-project
~/github/tanren/scripts/install.sh --profile python-uv
```

This installs:
- **Commands** into `.claude/commands/tanren/` and `.opencode/commands/tanren/`
- **Standards** from the selected profile into `tanren/standards/`
- **Product templates** into `tanren/product/`
- **Scripts** (audit tools) into `tanren/scripts/`

## Quick Start — Worker Manager

The worker-manager is a host-level service that polls for dispatches, spawns agent processes, and writes results.

```bash
cd worker-manager
export WM_IPC_DIR=~/data/ipc/main
export WM_GITHUB_DIR=~/github
uv run worker-manager
```

See [worker-manager/README.md](worker-manager/README.md) for full configuration.

## Structure

```
tanren/
├── commands/        # 15 command files for AI agent workflows
├── scripts/         # Audit scripts
├── templates/       # Starter templates for product docs, audits, Makefile
├── profiles/        # Standards profiles (default, python-uv)
├── worker-manager/  # Autonomous orchestration service
├── protocol/        # Agent communication protocol spec
└── config.yml       # Framework configuration
```

## Profiles

- **default** — Minimal language-agnostic standards
- **python-uv** — Python + uv standards (typing, testing, architecture patterns)

## Commands

| Command | Purpose |
|---------|---------|
| shape-spec | Shape a spec from issue to implementation-ready branch |
| walk-spec | Walk through spec steps interactively |
| do-task | Execute a single task autonomously |
| audit-task | Audit completed task against standards |
| audit-spec | Audit a spec for completeness |
| run-demo | Run a demo of implemented features |
| investigate | Investigate persistent failures and propose fixes |
| sync-roadmap | Synchronize roadmap with current state |
| plan-product | Plan product features and priorities |
| handle-feedback | Process and route feedback |
| resolve-blockers | Resolve blocking issues |
| discover-standards | Discover and propose new standards |
| inject-standards | Inject standards into the project |
| index-standards | Rebuild the standards index |
| triage-audits | Triage audit findings |

## Configuration

Three distinct scopes — never mixed:

- **Developer-scoped** — credentials, CLI auth, personal preferences. Never committed to git. Stored locally or via org vault.
- **Project-scoped** — `tanren.yml`, CI config, shared non-secret env vars. Committed to git. Declares what is needed, never contains secret values.
- **Organization-scoped** — adapter selection, permitted models, resource limits, access control policies.

## Execution Environments

The execution environment is where agent work happens. The abstraction supports local subprocesses, Docker containers, or remote VMs via SSH.

```python
class ExecutionEnvironment(Protocol):
    async def provision(dispatch, config) -> EnvironmentHandle: ...
    async def execute(handle, dispatch, config) -> PhaseResult: ...
    async def get_access_info(handle) -> AccessInfo: ...
    async def teardown(handle) -> None: ...
```

See [worker-manager/ADAPTERS.md](worker-manager/ADAPTERS.md) for the full adapter reference.

## Install Modes

- **Fresh install**: Creates `tanren/` directory with templates, standards, scripts, and commands
- **Update**: Refreshes only scripts and commands (preserves project-specific content)

## Documentation

- [PROTOCOL.md](protocol/PROTOCOL.md) — IPC protocol specification (dispatch/result schemas, state machines, concurrency model)
- [worker-manager/README.md](worker-manager/README.md) — Worker manager architecture and configuration
- [worker-manager/ADAPTERS.md](worker-manager/ADAPTERS.md) — Adapter protocol reference and extension guide
