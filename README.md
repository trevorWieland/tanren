# tanren

Opinionated orchestration engine for agentic software development.

Tanren decides **what work happens and in what order** (issue intake, spec
lifecycle, orchestration, gates, feedback). Agent runtimes decide **how each
role executes** (CLI/model/auth/tooling).

## What Tanren Is

Tanren has two coupled halves:

1. **Execution framework** (`packages/tanren-core/`, `services/`): dispatch routing, environment
   provisioning, retries, lifecycle handling, and result emission.
2. **Methodology system** (`commands/`, `profiles/`, `templates/`):
   reusable agent instructions, standards, and product context.

## What Tanren Is Not

- Not a model router or model chooser
- Not tied to one coordinator UX (dashboard/CLI/chat can all sit above tanren)
- Not a vendor-locked hosted platform

## The Three Layers

```
Coordinator -> Tanren -> Agent Runtime
```

- **Coordinator**: identity, authorization, developer interface, reporting
- **Tanren**: workflow state machine and orchestration policy
- **Agent runtime**: role mapping to CLI/model/auth configuration

## Quick Start

### Install Tanren into a Project

```bash
cd your-project
~/github/tanren/scripts/install.sh --profile python-uv
```

Installs commands, standards, product templates, and helper scripts.

### Run Worker Manager

```bash
export WM_IPC_DIR=~/data/ipc/main
export WM_GITHUB_DIR=~/github
uv run worker-manager
```

## Repository Structure

```text
tanren/
├── commands/        # 15 workflow command files
├── profiles/        # standards profiles (default, python-uv)
├── templates/       # product/audit/bootstrap templates
├── packages/
│   └── tanren-core/ # core orchestration library
├── services/
│   ├── tanren-api/  # HTTP API (FastAPI)
│   ├── tanren-cli/  # CLI tool
│   └── tanren-daemon/ # worker manager daemon
├── protocol/        # file-based IPC protocol specification
├── docs/            # architecture, workflow, ops, roadmap
└── scripts/         # install and utility scripts
```

## Commands

`shape-spec`, `do-task`, `audit-task`, `run-demo`, `audit-spec`, `walk-spec`,
`handle-feedback`, `resolve-blockers`, `investigate`, `plan-product`,
`discover-standards`, `inject-standards`, `index-standards`, `triage-audits`,
`sync-roadmap`

## Profiles

- `default`: minimal language-agnostic standards
- `python-uv`: strict Python + uv standards (typing/testing/architecture)

## Configuration Scopes

- **Developer-scoped**: local auth/secrets/preferences (never committed)
- **Project-scoped**: repo config (`tanren.yml`, standards, product docs)
- **Organization-scoped**: runtime policy and infrastructure config

## Execution Environments

The `ExecutionEnvironment` abstraction supports local and remote lifecycle:
`provision() -> execute() -> get_access_info() -> teardown()`

See `docs/ADAPTERS.md` for protocol details.

## Docker

The `tanren-api` image is published to GHCR as a single all-inclusive image.
Adapter selection (Hetzner, GCP, GitHub, Linear) is a runtime config decision —
all optional deps are included by default (~20-30 MB overhead).

On merge to master the release workflow auto-bumps the patch version and
publishes with tags: `latest`, `{version}`, `sha-{short}`.

```bash
# Build locally
make docker           # tanren-api:latest (all adapters)
make docker-slim      # tanren-api:slim   (no optional adapters)
```

## Documentation

- `docs/README.md` - documentation index
- `docs/architecture/overview.md` - architecture and boundaries
- `docs/workflow/spec-lifecycle.md` - lifecycle and orchestration rules
- `docs/getting-started/bootstrap.md` - install/bootstrap flow
- `docs/operations/security-secrets.md` - security and secret handling
- `docs/operations/observability.md` - events and metering
- `docs/interfaces.md` - CLI/library/IPC interaction surfaces
- `docs/design-principles.md` - architectural principles
- `docs/roadmap.md` - date-stamped roadmap
- `protocol/README.md` + `protocol/PROTOCOL.md` - protocol overview and full spec
- `docs/worker-manager-README.md` - runtime behavior and operations
- `docs/ADAPTERS.md` - adapter architecture and extension points
