# Dispatch Lifecycle

The dispatch orchestrator (`dispatch_orchestrator.py`) is the single source of
truth for all dispatch operations. CLI, MCP, and REST API all call the same
orchestration functions -- no entry point implements its own lifecycle logic.

## State Machine

Every dispatch follows a simple status progression:

```
PENDING ──► RUNNING ──► COMPLETED
                   └──► FAILED
                   └──► CANCELLED
```

- **PENDING** -- dispatch created, initial step enqueued.
- **RUNNING** -- at least one step is executing.
- **COMPLETED** -- all steps finished successfully.
- **FAILED** -- a step failed (and no retry succeeded).
- **CANCELLED** -- dispatch cancelled by user or system.

Defined in `store/enums.py` as `DispatchStatus`.

## Step Types

Each dispatch consists of ordered steps. Step types and their sequencing:

| Step type | Sequence | Lane | Purpose |
|-----------|----------|------|---------|
| `provision` | 0 | `None` | Create execution environment (VM, container, local) |
| `execute` | 1+ | impl/audit/gate | Run the agent CLI in the provisioned environment |
| `teardown` | last | `None` | Destroy the execution environment |
| `dry_run` | 0 | `None` | Validate config without provisioning |

Steps have their own status enum (`StepStatus`): PENDING, RUNNING, COMPLETED,
FAILED, CANCELLED.

### Lanes

Execute steps are assigned to concurrency lanes based on the CLI tool:

| CLI | Lane |
|-----|------|
| `claude`, `opencode` | `impl` |
| `codex` | `audit` |
| `bash` | `gate` |

Lane limits are configured via `WM_MAX_IMPL`, `WM_MAX_AUDIT`, `WM_MAX_GATE`.
Provision and teardown steps share a separate `WM_MAX_PROVISION` limit.

## Dispatch Modes

| Mode | Behavior |
|------|----------|
| `AUTO` | Worker auto-chains steps: provision -> execute -> teardown. Used by CLI `tanren run` and full-lifecycle API. |
| `MANUAL` | Caller drives each step individually via separate API calls. Used by the provision/execute/teardown REST endpoints and MCP tools. |

## Guard Rules

The orchestrator enforces state guards before enqueueing steps. Guards inspect
existing step projections and raise typed errors:

| Error | Trigger | Meaning |
|-------|---------|---------|
| `ConcurrentExecuteError` | Enqueue execute when another execute is PENDING or RUNNING | Only one execute step may be active at a time |
| `PostTeardownExecuteError` | Enqueue execute when a teardown is PENDING, RUNNING, or COMPLETED | Once teardown is requested, no more executes are allowed |
| `ActiveExecuteTeardownError` | Enqueue teardown when an execute is PENDING or RUNNING | Must wait for active execute to finish before teardown |
| `DuplicateTeardownError` | Enqueue teardown when one is already PENDING, RUNNING, or COMPLETED | Prevents duplicate teardown. Exception: `allow_retry_after_failure=True` permits retry when all prior teardowns FAILED |

All guard errors inherit from `DispatchGuardError`. Guard checks can be
bypassed with `check_guards=False` for internal use (e.g., auto-mode chaining).

## Orchestration Functions

| Function | Creates dispatch? | Step type | Returns |
|----------|-------------------|-----------|---------|
| `create_dispatch()` | Yes | `provision` | `DispatchResult` |
| `enqueue_execute_step()` | No | `execute` | `StepEnqueueResult` |
| `enqueue_teardown_step()` | No | `teardown` | `StepEnqueueResult` |
| `enqueue_dry_run_step()` | Yes | `dry_run` | `DispatchResult` |
| `get_provision_result()` | No | -- | `ProvisionResult` |

`create_dispatch` and `enqueue_dry_run_step` both create a dispatch projection,
append a `DispatchCreated` event, and enqueue the initial step.

## Input Resolution Pipeline

Before calling the orchestrator, inputs must be resolved through the builder:

```
ConfigResolver          dispatch_builder           dispatch_orchestrator
     │                       │                            │
     │  load_tanren_config   │                            │
     │  load_project_env     │                            │
     │──────────────────────►│  resolve_dispatch_inputs   │
     │                       │  resolve_provision_inputs   │
     │                       │  resolve_cli_auth           │
     │                       │───────────────────────────►│  create_dispatch
     │                       │                            │  enqueue_execute_step
     │                       │                            │  enqueue_teardown_step
```

### ConfigResolver

Protocol with two implementations (see `config_resolver.py`):

| Implementation | Source | Used by |
|---------------|--------|---------|
| `DiskConfigResolver` | Local git checkout filesystem | CLI, daemon |
| `GitHubConfigResolver` | GitHub raw content API | API, MCP |

Both load `tanren.yml` (project config) and `.env` (project env vars).
Infrastructure config (`remote.yml`, `roles.yml`) is loaded separately via
`WorkerConfig` and is not part of this protocol.

### Builder Functions

`dispatch_builder.py` provides two main resolution functions:

- **`resolve_dispatch_inputs()`** -- full resolution: profile, env, secrets,
  CLI/auth/model, and gate command. Used for execute-phase dispatches.
- **`resolve_provision_inputs()`** -- subset: profile, env, secrets only.
  Used when provisioning without knowing the execute phase yet.

Both return a `ResolvedInputs` dataclass. Each input can be pre-resolved by
the caller to skip redundant resolution.

## Error Mapping

Entry points map orchestrator errors to their own error types:

| Entry point | Guard error handling |
|-------------|---------------------|
| CLI | Catches `DispatchGuardError`, prints message, exits non-zero |
| REST API | Catches `DispatchGuardError`, raises `ServiceError` (HTTP 500) |
| MCP | Same service layer as REST API |

See `services/tanren-api/src/tanren_api/services/run.py` for the API mapping
and `services/tanren-cli/src/tanren_cli/run_cli.py` for the CLI handling.
