# Worker Manager

The worker manager is a host-level service that bridges a coordinating agent and
the AI coding agents that do the actual work. It polls an IPC directory for
dispatch files, routes each dispatch into one of three role-based queues, spawns
the appropriate CLI process inside a git worktree, extracts structured signals
from agent output, runs pre/post-flight integrity checks, and writes results
back through the IPC layer.


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
| `WM_IPC_DIR` | `~/github/nanoclaw/data/ipc/discord_main` | IPC directory path for the coordinator group |
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


## Roles Configuration

When `WM_ROLES_CONFIG_PATH` is set, the worker manager reads a YAML file that
maps workflow roles to specific CLI tools, models, and auth methods. This
allows different phases to use different backends.

```yaml
agents:
  conversation:
    cli: claude-code
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
    cli: claude-code
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
this lifecycle, running agents as local subprocesses in git worktrees. The
protocol is designed to also support Docker containers or remote VMs in the
future.


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
`{timestamp_ms}-{random6}.json`. Nudge files are wrapped in NanoClaw's IPC
envelope: `{"type": "message", "text": "<nudge-json>"}`.


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

1. Process environment
2. `~/.aegis/secrets.env` (managed via `tanren secret set`)
3. Project `.env` file
4. Defaults from tanren.yml optional vars

The `tanren env check` command validates the current environment locally.
The `tanren env init` command scaffolds an env block from `.env.example`.

### Secret Management

```bash
tanren secret set OPENROUTER_API_KEY sk-or-v1-...
tanren secret list
```

Secrets are stored in `~/.aegis/secrets.env` and automatically loaded during
environment validation.


## Development

The worker manager uses [uv](https://docs.astral.sh/uv/) for dependency
management and requires Python 3.14+.

```bash
cd worker-manager
uv sync                      # Install dependencies
make check                   # Lint (ruff) + unit tests
make ci                      # check + integration tests
make format                  # Auto-format with ruff
make test                    # Unit tests only
```

### Source Layout

```
src/worker_manager/
  __main__.py           # Entry point (asyncio.run)
  cli.py                # tanren CLI (env check, secret set/list)
  config.py             # WM_ env var configuration
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
