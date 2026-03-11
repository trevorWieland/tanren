# Worker Manager

The worker manager is a host-level service that bridges a coordinating agent and
the AI coding agents that do the actual work. It polls an IPC directory for
dispatch files, routes each dispatch into one of three role-based queues, spawns
the appropriate CLI process inside a git worktree, extracts structured signals
from agent output, runs pre/post-flight integrity checks, and writes results
back through the IPC layer.

## Documentation Boundaries

- This README is the canonical runtime and operations reference for worker-manager.
- Project-level architecture and lifecycle policy live in `../docs/`.
- Protocol wire contracts live in `../protocol/PROTOCOL.md`.


## Architecture

### Dispatch Routing

When a dispatch arrives the router inspects its `cli` field and places it into
one of three lanes:

| Lane | CLI types | Default concurrency | Purpose |
|------|-----------|---------------------|---------|
| **impl** | `opencode`, `claude` | 1 | Implementation and modification work |
| **audit** | `codex` | 1 | Code review and spec auditing |
| **gate** | `bash` | 3 (parallel) | Test/lint/build gate checks |

The impl and audit lanes use serial consumers (one process at a time) because
they share worktree state. The gate lane uses a parallel consumer with a
semaphore, allowing concurrent gate checks across different specs.

### Adapter Pattern

All external interactions are behind protocol interfaces (`adapters/protocols.py`).
The manager accepts injected adapters or falls back to concrete defaults:

| Protocol | Default | Responsibility |
|----------|---------|----------------|
| `WorktreeManager` | `GitWorktreeManager` | Create, register, clean up git worktrees |
| `PreflightRunner` | `GitPreflightRunner` | Branch validation, file snapshots |
| `PostflightRunner` | `GitPostflightRunner` | Integrity checks, git push |
| `ProcessSpawner` | `SubprocessSpawner` | Spawn CLI processes with timeout |
| `EnvValidator` | `DotenvEnvValidator` | Validate tanren.yml env requirements |
| `EnvProvisioner` | `DotenvEnvProvisioner` | Provision .env files in worktrees |
| `EventEmitter` | `NullEventEmitter` / `SqliteEventEmitter` | Structured observability |
| `ExecutionEnvironment` | `LocalExecutionEnvironment` | Full lifecycle wrapper |

### Dispatch Lifecycle

1. **Poll** -- Scan `dispatch/` every `WM_POLL_INTERVAL` seconds.
2. **Route** -- Delete dispatch file (read-once), place in queue by CLI type.
3. **Setup** -- Create git worktree, provision `.env`, register in `worktrees.json`.
4. **Work phases** -- Environment validation, pre-flight, heartbeat start, process
   spawn with retry (3x transient at 10/30/60s backoff, 1x ambiguous at 10s),
   signal extraction, outcome mapping, plan metrics, post-flight/push, findings
   parsing, heartbeat stop.
5. **Result** -- Write to `results/`, send nudge to `input/`.
6. **Cleanup** -- Remove worktree and registry entry.


## Configuration

All configuration is read from environment variables with the `WM_` prefix.

| Variable | Default | Description |
|----------|---------|-------------|
| `WM_IPC_DIR` | *(required)* | IPC directory path for the coordinator group |
| `WM_GITHUB_DIR` | `~/github` | Root directory containing git repositories |
| `WM_COMMANDS_DIR` | `.claude/commands/tanren` | Relative path to tanren commands within a project |
| `WM_POLL_INTERVAL` | `5.0` | Seconds between dispatch directory polls |
| `WM_HEARTBEAT_INTERVAL` | `30.0` | Seconds between heartbeat file updates |
| `WM_OPENCODE_PATH` | `opencode` | Path to opencode CLI binary |
| `WM_CODEX_PATH` | `codex` | Path to codex CLI binary |
| `WM_CLAUDE_PATH` | `claude` | Path to Claude Code CLI binary |
| `WM_DATA_DIR` | `~/.local/share/tanren-worker` | Directory for worker manager runtime state |
| `WM_WORKTREE_REGISTRY_PATH` | `{data_dir}/worktrees.json` | Path to the worktree registry file |
| `WM_MAX_OPENCODE` | `1` | Max concurrent implementation (opencode/claude) processes |
| `WM_MAX_CODEX` | `1` | Max concurrent audit (codex) processes |
| `WM_MAX_GATE` | `3` | Max concurrent gate (bash) processes |
| `WM_EVENTS_DB` | *(none)* | SQLite events DB path; enables event emission when set |
| `WM_ROLES_CONFIG_PATH` | *(none)* | Path to roles YAML config file |
| `WM_REMOTE_CONFIG` | *(none)* | Path to `remote.yml`; enables remote VM execution when set |


## Roles Configuration

When `WM_ROLES_CONFIG_PATH` is set, the worker manager reads a YAML file that
maps workflow roles to specific CLI tools, models, and auth methods. This
allows different phases to use different backends.

```yaml
agents:
  conversation:
    cli: claude
    model: claude-opus-4-6
    auth: oauth
  implementation:
    cli: opencode
    model: custom-model
    endpoint: https://llm.internal.company.com/v1
    auth: api_key
  audit:
    cli: codex
    auth: api_key
  default:
    cli: claude
    model: claude-sonnet-4-20250514
    auth: oauth
```

### Auth Models

Two authentication strategies are supported:

- **API key auth** (`auth: api_key`) -- Environment variable injection. The
  worker manager passes `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, or
  `OPENROUTER_API_KEY` (as declared in tanren.yml) into the spawned process
  environment.
- **OAuth/subscription auth** (`auth: oauth`) -- Session tokens from a browser
  login flow. Used by CLI tools that authenticate via subscription (e.g.,
  Claude Code with Max subscription).


## Execution Environments

The `ExecutionEnvironment` protocol defines where and how agent work runs:

```
provision() --> execute() --> get_access_info() --> teardown()
```

- **`provision()`** -- Validate env, run pre-flight, return `EnvironmentHandle`.
- **`execute()`** -- Run agent with heartbeat + retry, return `PhaseResult`.
- **`get_access_info()`** -- Return connection details for debugging.
- **`teardown()`** -- Clean up resources.

The default `LocalExecutionEnvironment` wraps the fine-grained adapters into
this lifecycle, running agents as local subprocesses in git worktrees.

When `WM_REMOTE_CONFIG` is set, the manager automatically constructs an
`SSHExecutionEnvironment` that runs agents on remote VMs via SSH. See
[ADAPTERS.md](ADAPTERS.md) for the full sub-adapter decomposition.

### Remote Execution Setup

1. Create a `remote.yml` config with `provisioner: {type, settings}` (see [ADAPTERS.md](ADAPTERS.md) for schema)
2. Set `WM_REMOTE_CONFIG=/path/to/remote.yml`
3. Ensure SSH key access to your VMs
4. Provide git/cloud tokens via shell env or `remote.yml -> secrets.developer_secrets_path`
   (the worker loads that file when remote execution initializes)
5. If using Hetzner, install optional dependency: `uv sync --extra hetzner`

### VM Management

```bash
tanren vm list      # Show active VM assignments
tanren vm release VM_ID  # Manually release a stuck VM
tanren vm recover   # Check connectivity, release unreachable VMs
tanren vm dry-run --project my-project --environment-profile default
```

All CLI commands are implemented with Typer and keep stable command/flag
surface for `tanren env`, `tanren secret`, `tanren vm`, and `tanren run`.

```bash
tanren run provision --project my-project --environment-profile default --branch main
tanren run execute --handle <env_id|vm_id> --project my-project --spec-path tanren/specs/s0001 --phase do-task
tanren run teardown --handle <env_id|vm_id>
tanren run full --project my-project --environment-profile default --branch main --spec-path tanren/specs/s0001 --phase do-task
```

Run handles now persist a wall-clock `provisioned_at_utc` timestamp.
Handle files using older schema are rejected; re-run `tanren run provision`.

On startup, the manager automatically runs recovery to release VMs from
previous crashed sessions.


## Running

```bash
cd worker-manager
uv run worker-manager
```

The manager handles `SIGTERM`/`SIGINT` for graceful shutdown: it stops the poll
loop, cancels all consumer tasks, closes the event emitter, and exits cleanly.

### Health File

Each poll cycle writes `worker-health.json` to the IPC directory with `alive`,
`pid`, `started_at`, `last_poll`, `active_processes`, and `queued_dispatches`.
The coordinator detects worker crashes via stale `last_poll` timestamps.

### Heartbeats

During active work phases a `.heartbeat` file is written per dispatch into
`in-progress/`, containing a Unix timestamp updated every
`WM_HEARTBEAT_INTERVAL` seconds. Stale heartbeats (>60s) are cleaned on startup.


## Adapters

Each adapter implements a `Protocol` from `adapters/protocols.py` and can be
swapped via constructor injection for testing or alternative backends.

- **GitWorktreeManager** -- Worktrees at `{github_dir}/{project}-wt-{issue}`,
  registry in `worktrees.json`, cleanup on completion.
- **GitPreflightRunner** -- Branch validation, file snapshots/hashes, status clearing.
- **GitPostflightRunner** -- Spec-revert detection, integrity repairs, git push
  for push phases (do-task, audit-task, run-demo, audit-spec).
- **SubprocessSpawner** -- Builds CLI commands via `process.spawn_process()`.
- **DotenvEnvValidator** -- Loads tanren.yml, reads env layers, validates vars.
- **DotenvEnvProvisioner** -- Copies `.env` into worktrees during setup.


## Event System

Two emitter backends: **NullEventEmitter** (default, discards events) and
**SqliteEventEmitter** (enabled via `WM_EVENTS_DB`, persists to SQLite with
indexes on workflow_id, event_type, and timestamp).

| Event | Emitted When |
|-------|-------------|
| `DispatchReceived` | Dispatch picked up from poll loop |
| `PhaseStarted` | Agent/gate process about to spawn |
| `PhaseCompleted` | Phase finished (outcome, signal, duration) |
| `PreflightCompleted` | Pre-flight checks done (pass/fail, repairs) |
| `PostflightCompleted` | Post-flight integrity checks done (pushed, repairs) |
| `ErrorOccurred` | Unhandled error during dispatch handling |
| `RetryScheduled` | Transient/ambiguous error triggered retry |
| `VMProvisioned` | VM acquired for remote execution |
| `VMReleased` | VM released after workflow completion |
| `BootstrapCompleted` | VM bootstrap finished (installed/skipped tools) |

All events carry `timestamp` (ISO 8601) and `workflow_id`.


## IPC Protocol

Communication uses a shared filesystem IPC directory:

| Directory | Direction | Purpose |
|-----------|-----------|---------|
| `dispatch/` | Coordinator --> Worker | Dispatch files (JSON, read-once) |
| `results/` | Worker --> Coordinator | Result files with outcome, signal, findings |
| `in-progress/` | Worker --> Coordinator | Heartbeat files for crash detection |
| `input/` | Worker --> Coordinator | Nudge files that wake the coordinator |

Files use atomic writes (`.tmp` + fsync + rename). Filenames:
`{timestamp_ms}-{random6}.json`. Nudge files are wrapped in the coordinator's
IPC envelope: `{"type": "message", "text": "<nudge-json>"}`.


## tanren.yml Environment Schema

Projects declare their environment requirements in `tanren.yml`. The worker
manager validates these before spawning any agent process.

```yaml
env:
  on_missing: error          # error | warn | prompt
  required:
    - key: OPENROUTER_API_KEY
      description: "OpenRouter API key"
      pattern: "^sk-or-v1-"
      hint: "Get one at https://openrouter.ai/keys"
  optional:
    - key: LOG_LEVEL
      description: "Log verbosity"
      default: "INFO"
```

Environment variables are loaded from multiple layers (highest priority first):

1. Process environment (`os.environ`)
2. Project `.env` file
3. `~/.config/tanren/secrets.d/*.env` files (alphabetical order)
4. `~/.config/tanren/secrets.env` (managed via `tanren secret set`)
5. Defaults from tanren.yml optional vars

The `tanren env check` command validates the current environment locally.
The `tanren env init` command scaffolds an env block from `.env.example`.

### Secret Management

```bash
tanren secret set OPENROUTER_API_KEY sk-or-v1-...
tanren secret list
```

Secrets are stored in `~/.config/tanren/secrets.env` (or `$XDG_CONFIG_HOME/tanren/secrets.env`)
and automatically loaded during environment validation.

For canonical secret scope, config scope, and security model guidance, see
`../docs/operations/security-secrets.md`.


## Development

The worker manager uses [uv](https://docs.astral.sh/uv/) for dependency
management and requires Python 3.14+.

```bash
cd worker-manager
uv sync                      # Install dependencies
make check                   # Lint (ruff) + typecheck (ty) + unit tests
make ci                      # check + integration tests (no real SSH)
make format                  # Auto-format with ruff
make test                    # Unit tests only
make integration             # Integration tests (excludes SSH/local_env)
make integration-ssh         # SSH integration tests (requires --ssh-host)
make integration-local       # Local environment integration tests
```

### Source Layout

```
src/worker_manager/
  __main__.py           # Entry point (asyncio.run)
  cli.py                # tanren CLI (env, secret, vm, run subcommands)
  vm_cli.py             # tanren vm list/release/recover/dry-run
  run_cli.py            # tanren run provision/execute/teardown/full
  config.py             # WM_ env var configuration
  remote_config.py      # remote.yml loader
  secrets.py            # SecretLoader for remote injection
  manager.py            # Poll loop, dispatch handling, result writing
  queues.py             # 3-lane dispatch router with semaphores
  schemas.py            # Pydantic models (Dispatch, Result, Phase, Cli, ...)
  signals.py            # Signal extraction and outcome mapping
  errors.py             # Error classification (transient/fatal/ambiguous)
  heartbeat.py          # Crash-detection heartbeat writer
  ipc.py                # Atomic file I/O, dispatch scanning
  adapters/             # Protocol interfaces + concrete implementations
  env/                  # tanren.yml env validation, secrets, provisioning
tests/unit/             # make test
tests/integration/      # make ci
```

## Related Documentation

- `../docs/architecture/overview.md` - architecture boundaries and layering
- `../docs/workflow/spec-lifecycle.md` - lifecycle policy and orchestration intent
- `../docs/operations/observability.md` - event model and metering queries
- `../docs/interfaces.md` - CLI/library/IPC interaction surfaces
- `ADAPTERS.md` - adapter decomposition and extension points
