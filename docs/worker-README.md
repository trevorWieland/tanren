# Worker

The Worker is a queue-consuming daemon that processes dispatch steps from the
store, backed by SQLite or Postgres.

## Architecture

The Worker polls the `JobQueue` for pending steps, executes them via the
`ExecutionEnvironment` adapter, and acknowledges results back to the store.
All state transitions are recorded as events in the `EventStore`.

### Lane Consumers

Each step is assigned to a lane that controls concurrency:

| Lane | CLI types | Default concurrency | Purpose |
|------|-----------|---------------------|---------|
| **impl** | `opencode`, `claude` | 1 | Implementation and modification work |
| **audit** | `codex` | 1 | Code review and spec auditing |
| **gate** | `bash` | 3 (parallel) | Test/lint/build gate checks |
| **provision** | -- | 10 | Environment provisioning |

### Step Processing

Each dispatch is broken into steps (provision, execute, teardown) that are
processed sequentially:

1. **Dequeue** -- Worker claims a pending step from the `JobQueue` via atomic
   `dequeue()`, respecting per-lane concurrency limits.
2. **Execute** -- The step payload determines the action:
   - `provision`: Create execution environment via `ExecutionEnvironment.provision()`
   - `execute`: Run the agent phase via `ExecutionEnvironment.execute()`
   - `teardown`: Clean up via `ExecutionEnvironment.teardown()`
   - `dry_run`: Validate configuration without executing
3. **Ack/Nack** -- On success, `ack()` marks the step completed with its result.
   On failure, `nack()` either retries (transient errors, up to 3 retries with
   10s/30s/60s backoff) or marks the step failed.

### Auto-Chaining

For `AUTO` mode dispatches, the Worker uses `ack_and_enqueue()` to atomically
complete the current step and enqueue the next one in a single transaction.
This prevents race conditions and ensures the dispatch progresses without
gaps.

## Embedded CLI Mode

The CLI provides `run_until_step_complete()` for synchronous dispatch
execution. This submits a dispatch, enqueues steps, and polls until
completion -- useful for `tanren run full` and similar commands that need
to block until the work is done.

## Configuration

Worker configuration is managed via `WorkerConfig` (see
`packages/tanren-core/src/tanren_core/worker_config.py`). Key settings:

- **Store backend** -- SQLite (file path) or Postgres (connection string)
- **Poll interval** -- Seconds between queue polls
- **Lane concurrency** -- Max concurrent steps per lane
- **CLI paths** -- Paths to agent CLI binaries (opencode, codex, claude)

## Related Documentation

- [architecture/overview.md](architecture/overview.md) - architecture boundaries and layering
- [workflow/spec-lifecycle.md](workflow/spec-lifecycle.md) - lifecycle policy and orchestration intent
- [operations/observability.md](operations/observability.md) - event model and metering queries
- [interfaces.md](interfaces.md) - CLI, library, and store interaction surfaces
- [ADAPTERS.md](ADAPTERS.md) - adapter decomposition and extension points
