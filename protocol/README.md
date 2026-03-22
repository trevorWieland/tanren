# Protocol Overview

This folder provides a high-level overview of the tanren dispatch protocol.

## Queue-Based Architecture

Tanren uses an event-sourced queue model. Clients (API, MCP, CLI) submit
dispatches to a store (SQLite or Postgres). The Worker daemon polls the
store's job queue and processes steps.

### Store Protocols

The canonical protocol definitions live in
`packages/tanren-core/src/tanren_core/store/protocols.py`. Three protocols
define the contract between business logic and storage:

| Protocol | Responsibility |
|----------|----------------|
| `EventStore` | Append-only event log with transactional projection maintenance |
| `JobQueue` | Step-based job queue (enqueue, dequeue, ack, nack) backed by `step_projection` |
| `StateStore` | Read-only queries against dispatch and step projection tables |

### Dispatch Lifecycle

1. **Submit** -- Client creates a dispatch via API/MCP/CLI. A
   `dispatch_projection` row and initial step are inserted into the store.
2. **Enqueue** -- Steps are enqueued into the `step_projection` table with
   `status='pending'` and a lane assignment (impl, audit, gate, provision).
3. **Dequeue** -- Worker claims a pending step atomically
   (`status='running'`), respecting per-lane concurrency limits.
4. **Execute** -- Worker processes the step (provision, execute, teardown,
   or dry-run) via the `ExecutionEnvironment` adapter.
5. **Ack/Nack** -- On success, the step is acknowledged with its result.
   On failure, it is either retried or marked failed.
6. **Auto-chain** -- For `AUTO` mode dispatches, `ack_and_enqueue()`
   atomically completes the current step and enqueues the next one.

### Lanes

| Lane | Purpose | Default concurrency |
|------|---------|---------------------|
| impl | Implementation work (opencode, claude) | 1 |
| audit | Code review and auditing (codex) | 1 |
| gate | Test/lint/build checks (bash) | 3 |
| provision | Environment provisioning | 1 |

## When To Update

Update protocol docs in the same PR as any change to:

- store protocol method signatures
- step types or dispatch modes
- lane definitions or concurrency semantics
- event types or projection schemas

Related docs:

- `../docs/interfaces.md`
- `../docs/workflow/spec-lifecycle.md`
- `../docs/worker-README.md`
