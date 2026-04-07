# Configuration

All tanren configuration is driven by environment variables. The daemon and CLI
load `WM_*` variables from `~/.config/tanren/tanren.env` (or
`$XDG_CONFIG_HOME/tanren/tanren.env`), with `os.environ` taking precedence.

## Core (`WM_*`)

Loaded by `WorkerConfig.from_env()` in `worker_config.py`. Config file values
are injected into `os.environ` by `load_config_env()` in `config.py`; existing
env vars are never overwritten.

### Required

| Variable | Description |
|----------|-------------|
| `WM_IPC_DIR` | IPC directory path for coordinator group |
| `WM_GITHUB_DIR` | Root directory containing git repositories |
| `WM_DATA_DIR` | Directory for worker runtime state |
| `WM_COMMANDS_DIR` | Relative path to tanren commands within a project |
| `WM_EVENTS_DB` | SQLite path or `postgresql://` URL for the unified store |
| `WM_WORKTREE_REGISTRY_PATH` | Path to `worktrees.json` registry file |
| `WM_POLL_INTERVAL` | Seconds between filesystem dispatch polls (legacy) |
| `WM_HEARTBEAT_INTERVAL` | Seconds between heartbeat file updates |
| `WM_OPENCODE_PATH` | Path to opencode CLI binary |
| `WM_CODEX_PATH` | Path to codex CLI binary |
| `WM_CLAUDE_PATH` | Path to Claude Code CLI binary |
| `WM_MAX_OPENCODE` | Max concurrent impl-lane steps (maps to `max_impl`) |
| `WM_MAX_CODEX` | Max concurrent audit-lane steps (maps to `max_audit`) |
| `WM_MAX_GATE` | Max concurrent gate-lane steps |

### Optional

| Variable | Default | Description |
|----------|---------|-------------|
| `WM_ROLES_CONFIG_PATH` | `None` | Path to `roles.yml` (CLI/auth/model resolution) |
| `WM_REMOTE_CONFIG` | `None` | Path to `remote.yml` (enables remote execution) |
| `WM_CCUSAGE_CLAUDE_CMD` | `npx ccusage` | Command for ccusage (Claude) |
| `WM_CCUSAGE_CODEX_CMD` | `npx @ccusage/codex` | Command for @ccusage/codex |
| `WM_CCUSAGE_OPENCODE_CMD` | `npx @ccusage/opencode` | Command for @ccusage/opencode |
| `WM_LOG_LEVEL` | `INFO` | Log level for the daemon process |

## API (`TANREN_API_*`)

Loaded by `APISettings` (pydantic-settings) in `services/tanren-api/src/tanren_api/settings.py`.
Supports env vars with the `TANREN_API_` prefix and `.env` file loading.

| Variable | Default | Description |
|----------|---------|-------------|
| `TANREN_API_HOST` | `0.0.0.0` | API server bind address |
| `TANREN_API_PORT` | `8000` | API server port |
| `TANREN_API_API_KEY` | `""` | API key for `X-API-Key` header authentication |
| `TANREN_API_WORKERS` | `1` | Number of uvicorn workers |
| `TANREN_API_LOG_LEVEL` | `info` | API server log level |
| `TANREN_API_CORS_ORIGINS` | `[]` | Allowed CORS origins (JSON list) |
| `TANREN_API_DB_URL` | `tanren.db` | SQLite path or `postgresql://` URL |

## Logging

| Variable | Default | Description |
|----------|---------|-------------|
| `TANREN_LOG_LEVEL` | `INFO` | Log level for the core library |
| `NO_COLOR` | -- | Disable colored log output when set to any value |

## Adapter-Specific

These are read at runtime by adapter implementations. They are typically set
in the host environment or injected as secrets.

### Hetzner Cloud

| Variable | Description |
|----------|-------------|
| `HCLOUD_TOKEN` | Hetzner Cloud API token (used by `HetznerVMProvisioner`) |

### GCP

| Variable | Description |
|----------|-------------|
| `GCP_SSH_PUBLIC_KEY` | SSH public key for GCP VM access (used by `GCPVMProvisioner`) |
| `GOOGLE_CLOUD_PROJECT` | GCP project ID (used in integration tests) |

### GitHub

| Variable | Description |
|----------|-------------|
| `GITHUB_TOKEN` | GitHub personal access token (used by `GitHubIssueTracker`) |

### Linear

| Variable | Description |
|----------|-------------|
| `LINEAR_API_KEY` | Linear API key (used by `LinearIssueTracker`) |

## Docker

Docker configuration is set via `DockerConfig` fields in the environment
profile (`tanren.yml`), not via environment variables. Relevant fields:

| Field | Default | Description |
|-------|---------|-------------|
| `image` | `ubuntu:24.04` | Docker image for containers |
| `socket_url` | `None` | Docker daemon socket URL (uses system default if unset) |
| `network` | `None` | Docker network to attach containers to |
| `extra_volumes` | `()` | Additional bind-mount volume specs |
| `extra_env` | `{}` | Extra environment variables injected into the container |
