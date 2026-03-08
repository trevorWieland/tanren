# NanoClaw-Tanren IPC Protocol Specification

Version: 1.1
Date: 2026-03-07

Protocol for autonomous code orchestration between a NanoClaw coordinator
container and a host-level worker manager, using file-based IPC with
polling, atomic writes, and delete-after-process semantics.

## Table of Contents

0. [Component Roles](#0-component-roles)
1. [File System Layout](#1-file-system-layout)
2. [Dispatch Schema](#2-dispatch-schema)
3. [Result Schema](#3-result-schema)
4. [Workflow State Schema](#4-workflow-state-schema)
5. [Top-Level State Machine](#5-top-level-state-machine)
6. [Orchestrating Sub-State Machine](#6-orchestrating-sub-state-machine)
7. [Concurrency Model](#7-concurrency-model)
8. [Worktree Management](#8-worktree-management)
9. [Timeout and Retry Specifications](#9-timeout-and-retry-specifications)
10. [Edge Case Handling](#10-edge-case-handling)
11. [Worked Example](#11-worked-example)

---

## 0. Component Roles

Three components participate in the protocol:

**Coordinator container** (Claude Max, persistent NanoClaw group)
- Runs inside a NanoClaw container with persistent session
- Handles interactive phases (shape-spec, walk-spec) over Discord
- Owns workflows.json — reads and writes on every state change
- Processes workflow updates: reads results/, writes dispatch/
- Two trigger paths for processing (see Result Notification)

**Worker manager** (host-level service, not a NanoClaw group)
- Runs on the host, outside any container
- Polls dispatch/ every 5s for new work
- Spawns CLI processes (opencode, codex, bash) in worktrees
- Manages worktree lifecycle (create, validate, remove)
- Writes results to results/ and nudges coordinator via input/
- Maintains heartbeat files for crash detection

**Workflow monitor** (NanoClaw scheduled task, 60s cron)
- Runs inside the coordinator container as a NanoClaw cron task
- Safety-net fallback: polls results/ every 60s for unprocessed results
- Detects stale dispatches and crashed workers via heartbeat checks
- Not the primary processing path (nudge is — see Section 1.1)

---

## 1. File System Layout

IPC files live under the coordinator's per-group IPC directory. The host
path is `data/ipc/{coordinator-group}/`; inside the container it mounts
at `/workspace/ipc/`.

```
data/ipc/{coordinator-group}/
  messages/         # existing — Discord messages (coordinator -> NanoClaw)
  tasks/            # existing — scheduled tasks
  input/            # existing — follow-up messages (NanoClaw -> coordinator)
  dispatch/         # NEW — coordinator writes, worker manager reads+deletes
  results/          # NEW — worker manager writes, coordinator reads+deletes
  in-progress/      # NEW — worker manager writes heartbeat files
  workflows.json    # NEW — coordinator-owned persistent state (not consumed)
```

| Directory/File | Writer | Reader | Lifecycle |
|---|---|---|---|
| `dispatch/` | Coordinator | Worker manager | Write once, read once, delete after pickup |
| `results/` | Worker manager | Coordinator (workflow monitor) | Write once, read once, delete after processing |
| `in-progress/` | Worker manager | Coordinator (workflow monitor) | Created per-dispatch, updated every 30s, deleted on completion |
| `workflows.json` | Coordinator | Coordinator only | Overwritten atomically on every state change |

### Container View

From inside the coordinator container:

```
/workspace/ipc/dispatch/       # coordinator writes dispatch files here
/workspace/ipc/results/        # workflow monitor reads result files here
/workspace/ipc/workflows.json  # coordinator reads/writes state here
```

### File Naming Convention

All dispatch and result files follow the NanoClaw convention established
in `ipc-mcp-stdio.ts`:

```
{timestamp}-{random6}.json
```

Where `{timestamp}` is `Date.now()` (millisecond Unix epoch) and
`{random6}` is 6 random alphanumeric characters.

Examples:
```
1741359700123-a3f2b8.json
1741360042456-c7d91e.json
```

### Atomic Write Protocol

All file writes use the write-tmp-rename pattern to prevent partial reads:

```
1. Write content to {filename}.tmp
2. fsync / flush
3. Rename {filename}.tmp -> {filename}
```

Readers MUST ignore `.tmp` files. A `.json` file's presence guarantees
complete, parseable content.

### 1.1 Result Notification (Nudge Mechanism)

Two paths deliver results to the coordinator. Both use the same
processing logic; idempotency ensures at-most-once semantics.

**Primary path (nudge, ~500ms latency)**: After writing a result file to
`results/`, the worker manager also writes a nudge file to `input/`:

```json
{ "type": "workflow_result", "workflow_id": "wf-rentl-144-1741359600" }
```

NanoClaw's host-level IPC watcher picks up the file from `input/` and
delivers it as a follow-up message to the coordinator's persistent
session. The coordinator processes the result immediately — reads
`results/`, advances `workflows.json`, writes the next `dispatch/` file.
Latency: ~500ms (NanoClaw's `input/` poll interval inside the
container).

**Fallback path (cron, 60s poll)**: The workflow monitor (NanoClaw
scheduled task) independently polls `results/` on every tick. Catches
missed nudges, handles coordinator restarts, and serves as crash
recovery.

**Idempotency**: Both paths use the same processing logic. The result
file is deleted after processing. If the nudge triggers first, the cron
finds nothing to do. If the cron triggers first (e.g., nudge was lost),
the nudge finds nothing to do.

---

## 2. Dispatch Schema

Coordinator writes one file per dispatch to `/workspace/ipc/dispatch/`.
Worker manager reads, processes, and deletes.

```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "do-task",
  "project": "rentl",
  "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
  "branch": "s0146-slug",
  "cli": "opencode",
  "model": "glm-5",
  "gate_cmd": null,
  "context": null,
  "timeout": 1800
}
```

### Field Definitions

| Field | Type | Description |
|---|---|---|
| `workflow_id` | string | Unique workflow identifier. Format: `wf-{project}-{issue}-{epoch}` where `{epoch}` is the Unix timestamp when the workflow was created. This ensures uniqueness even if the same issue is re-orchestrated after a HALT (different creation time = different epoch). |
| `phase` | enum | `setup`, `do-task`, `audit-task`, `run-demo`, `audit-spec`, `gate` |
| `project` | string | Project name (matches repo name) |
| `spec_folder` | string | Relative path from project root to spec folder |
| `branch` | string | Git branch name for this workflow |
| `cli` | enum | `opencode`, `codex`, `bash` |
| `model` | string\|null | Model identifier passed to CLI, null for gates |
| `gate_cmd` | string\|null | Shell command for gate phases, null for agent phases |
| `context` | string\|null | Extra context passed to the agent (gate errors, retry prompts) |
| `timeout` | integer | Maximum execution time in seconds |

### CLI Routing Table

Default routing. The coordinator populates `cli`, `model`, and `gate_cmd`
based on the phase:

| Phase | cli | model | gate_cmd | Subscription |
|---|---|---|---|---|
| do-task | opencode | glm-5 | null | Z.ai |
| audit-task | codex | gpt-5.3-codex | null | OpenAI Plus |
| run-demo | opencode | glm-5 | null | Z.ai |
| audit-spec | codex | gpt-5.3-codex | null | OpenAI Plus |
| setup | bash | null | null | none |
| gate | bash | null | `make check` or `make all` | none |

The `setup` dispatch tells the worker manager to create the worktree. No
CLI invocation — the worker manager handles it internally. The result
confirms `outcome: "success"` (worktree created) or `outcome: "error"`
(branch missing, conflict, etc.).

> The coordinator's CLAUDE.md must include instructions for populating
> dispatch fields from this routing table and from project-specific
> configuration (gate commands, model overrides).

### Context Field Usage

The `context` field carries the same information that `orchestrate.sh`
passes as `extra_context` to `invoke_agent`:

| Sub-State | Context Content |
|---|---|
| `do_task` (initial) | null |
| `do_task_gate_fix` | Gate failure output: `"GATE FAILURE — 'make check' FAILED. Fix these errors...\n\`\`\`\n{output}\n\`\`\`"` |
| `spec_gate_fix` | Spec gate failure output: `"The full verification gate (make all) failed after all tasks were completed...\n\`\`\`\n{output}\n\`\`\`"` |
| `demo_retry` | Forceful retry prompt demanding fix tasks or pass signal |
| `audit_spec_retry` | Forceful retry prompt demanding fix items or pass status |
| All others | null |

### Path Resolution

The worker manager resolves `spec_folder` relative to the worktree root:

```
~/github/{project}-wt-{issue_number}/{spec_folder}
```

For example:
```
~/github/rentl-wt-144/tanren/specs/2026-02-19-1531-s0146-slug
```

---

## 3. Result Schema

Worker manager writes one file per completed phase to
`data/ipc/{coordinator-group}/results/`. Coordinator's workflow monitor
reads, processes, and deletes.

```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "do-task",
  "outcome": "success",
  "signal": "complete",
  "exit_code": 0,
  "duration_secs": 342,
  "gate_output": null,
  "tail_output": null,
  "unchecked_tasks": 2,
  "plan_hash": "a3f2b8c1",
  "spec_modified": false
}
```

### Field Definitions

| Field | Type | Description |
|---|---|---|
| `workflow_id` | string | Matches the dispatch's workflow_id |
| `phase` | enum | Matches the dispatch's phase |
| `outcome` | enum | `success`, `fail`, `blocked`, `error`, `timeout` |
| `signal` | string\|null | Raw agent signal or null (gates, no signal) |
| `exit_code` | integer | Process exit code |
| `duration_secs` | integer | Wall-clock execution time |
| `gate_output` | string\|null | Last 100 lines of gate output (gate phases only) |
| `tail_output` | string\|null | Last 50 lines of output (non-success outcomes only) |
| `unchecked_tasks` | integer | Count of unchecked `- [ ] Task N` lines in plan.md after phase |
| `plan_hash` | string | MD5 of plan.md after phase (first 8 hex chars) |
| `spec_modified` | boolean | True if spec.md was modified and reverted |

### Outcome Mapping

The worker manager maps raw agent signals and exit codes to the
`outcome` enum:

| Raw Signal / Condition | outcome | signal (preserved) |
|---|---|---|
| `complete` | success | `complete` |
| `pass` | success | `pass` |
| `all-done` | success | `all-done` |
| `fail` | fail | `fail` |
| `blocked` | blocked | `blocked` |
| `error` | error | `error` |
| No signal + exit code 0 | success | null |
| No signal + nonzero exit | error | null |
| Process exceeded timeout | timeout | null |
| Gate exit code 0 | success | null |
| Gate nonzero exit code | fail | null |

### Worker Manager Signal Extraction

The worker manager handles signal extraction asymmetries across
different agent types transparently:

1. **Primary**: Read `.agent-status` file in the spec folder. Extract
   signal with pattern `{command}-status: {signal}` (same as
   `extract_signal` in orchestrate.sh).

2. **Fallback**: Grep stdout for the signal pattern (fragile but
   backwards-compatible with agents that don't write the status file).

3. **audit-spec special case**: Status comes from `audit.md` first line
   (`status: pass|fail`), not from `.agent-status`. The worker manager
   reads audit.md after the codex process exits and maps:
   - `status: pass` -> signal: `pass`, outcome: `success`
   - `status: fail` -> signal: `fail`, outcome: `fail`
   - `status: unknown` or missing -> signal: null, outcome: `error`

4. **spec.md integrity**: After every agent phase, compute MD5 of
   spec.md. If it differs from the pre-phase snapshot:
   - Revert spec.md from backup
   - `git add spec.md && git commit --amend --no-edit`
   - Set `spec_modified: true` in the result

5. **plan.md metrics**: After every phase, compute:
   - `unchecked_tasks`: `grep -cP '^\s*- \[ \] Task \d+' plan.md`
   - `plan_hash`: first 8 hex chars of `md5sum plan.md`

---

## 4. Workflow State Schema

Coordinator-owned persistent state file. Written atomically to
`/workspace/ipc/workflows.json` on every state transition. Survives
container restarts.

```json
{
  "version": 1,
  "updated_at": "2026-03-07T15:46:55Z",
  "workflows": {
    "wf-rentl-144-1741359600": {
      "project": "rentl",
      "issue": 144,
      "branch": "s0146-slug",
      "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
      "state": "orchestrating",
      "sub_state": "task_gate",
      "dispatched": {
        "phase": "gate",
        "dispatched_at": "2026-03-07T15:40:00Z",
        "dispatch_file": "1741362000123-x7k9m2.json"
      },
      "cycle": 2,
      "retries": {
        "task_attempts": 1,
        "gate_attempt": 0,
        "demo_retry": 0,
        "audit_retry": 0
      },
      "staleness": {
        "count": 0,
        "prev_plan_hash": "a3f2b8c1"
      },
      "worktree_ready": true,
      "current_task": "Task 5: Implement batch progress tracking",
      "pre_phase_unchecked": 2,
      "history": [
        {
          "phase": "do-task",
          "outcome": "success",
          "signal": "complete",
          "duration_secs": 342,
          "ts": "2026-03-07T15:35:00Z"
        },
        {
          "phase": "gate",
          "outcome": "success",
          "signal": null,
          "duration_secs": 87,
          "ts": "2026-03-07T15:37:00Z"
        }
      ]
    }
  }
}
```

### Field Definitions

| Field | Type | Description |
|---|---|---|
| `version` | integer | Schema version (currently 1) |
| `updated_at` | ISO 8601 | Last modification timestamp |
| `workflows` | object | Map of workflow_id to workflow state |

### Per-Workflow Fields

| Field | Type | Description |
|---|---|---|
| `project` | string | Project name |
| `issue` | integer | GitHub issue number |
| `branch` | string | Git branch |
| `spec_folder` | string | Relative spec folder path |
| `state` | enum | Top-level state (see Section 5) |
| `sub_state` | enum\|null | Orchestrating sub-state (see Section 6), null when not orchestrating |
| `dispatched` | object\|null | Currently dispatched phase info, null when idle |
| `dispatched.phase` | string | Phase name |
| `dispatched.dispatched_at` | ISO 8601 | When the dispatch file was written |
| `dispatched.dispatch_file` | string | Filename of the dispatch file (for pickup detection) |
| `cycle` | integer | Current cycle number (increments on task_select re-entry) |
| `retries` | object | Retry counters |
| `retries.task_attempts` | integer | Attempts on the current task (resets when task changes) |
| `retries.gate_attempt` | integer | Gate retry count within current task (resets per task) |
| `retries.demo_retry` | integer | Consecutive demo retries without task addition |
| `retries.audit_retry` | integer | Consecutive audit-spec retries without task addition |
| `staleness.count` | integer | Consecutive cycles with unchanged plan_hash |
| `staleness.prev_plan_hash` | string | Plan hash from previous cycle's task_select |
| `worktree_ready` | boolean | Whether the worktree has been created. Default `false`. Set to `true` when the setup result succeeds. The workflow monitor MUST NOT enter `task_select` until `worktree_ready` is true. |
| `current_task` | string\|null | Label of the task currently being worked |
| `pre_phase_unchecked` | integer | Unchecked task count before current dispatched phase |
| `history` | array | Completed phase records (append-only within a workflow) |

---

## 5. Top-Level State Machine

```
idle -> shaping -> await_confirm -> orchestrating -> await_walk -> walking -> pr_review -> completed
                                        |                                       |
                                        |          (walk finds issues)           |
                                        +<--------------------------------------+
                                        |
                                        v
                                      halted
```

### State Descriptions

| State | Description | Actor |
|---|---|---|
| `idle` | No active work. Workflow not yet started or completed. | — |
| `shaping` | shape-spec running interactively in coordinator container over Discord. | Coordinator |
| `await_confirm` | Spec shaped, waiting for human confirmation to proceed. | Human |
| `orchestrating` | Autonomous implementation loop. Sub-state machine active. | Workflow monitor + Worker manager |
| `await_walk` | Orchestration complete. Waiting for human to start walk-spec. | Human |
| `walking` | walk-spec running interactively in coordinator container over Discord. | Coordinator + Human |
| `pr_review` | PR created, awaiting merge. | Human |
| `halted` | Unrecoverable error during orchestrating. Needs human intervention. Can resume. | Human |
| `completed` | PR merged, workflow finished. | — |

### Transition Table

| Current State | Event | Next State | Action |
|---|---|---|---|
| `idle` | Human: `@Aegis shape {project} #{issue}` | `shaping` | Coordinator starts shape-spec interactively |
| `shaping` | shape-spec completes | `await_confirm` | Coordinator asks human to confirm |
| `shaping` | shape-spec error/abort | `idle` | Clean up, notify human |
| `await_confirm` | Human confirms | `orchestrating` | Set sub_state=`worktree_setup`, dispatch setup phase |
| `await_confirm` | Human rejects | `idle` | Notify, no cleanup needed |
| `orchestrating` | Sub-state machine reaches COMPLETE | `await_walk` | Post to Discord: "Orchestration complete for #N. Say 'walk' to start walk-spec or 'defer' to do it later." |
| `orchestrating` | Sub-state machine reaches HALT | `halted` | Notify human via Discord with halt reason |
| `orchestrating` | Human cancels | `idle` | Clean up worktree, notify |
| `await_walk` | Human says "walk" | `walking` | Coordinator starts walk-spec interactively |
| `await_walk` | Human says "defer" | `await_walk` | Stays, reminder posted later |
| `await_walk` | Human cancels | `idle` | Clean up worktree, notify |
| `walking` | walk-spec passes | `pr_review` | Coordinator creates PR |
| `walking` | walk-spec finds issues | `orchestrating` | Set sub_state=`task_select`, tasks already added by walk-spec |
| `pr_review` | PR merged | `completed` | Clean up worktree, archive workflow |
| `pr_review` | PR changes requested | `orchestrating` | Set sub_state=`task_select` with change-request tasks |
| `halted` | Human: `@Aegis resume {workflow_id}` | `orchestrating` | Resume from last sub_state or task_select |
| `halted` | Human cancels | `idle` | Clean up worktree |

---

## 6. Orchestrating Sub-State Machine

The core of the protocol. Maps the control flow of `orchestrate.sh`
(lines 485-906) into dispatchable states that the workflow monitor
advances one phase at a time.

### Sub-States

| Sub-State | Description | Dispatches |
|---|---|---|
| `worktree_setup` | Worker manager creates git worktree | setup |
| `task_select` | Decision point: count tasks, check limits, pick next task | Nothing (immediate transition) |
| `do_task` | Agent implements the next unchecked task | do-task |
| `do_task_gate_fix` | Agent fixes gate failures from a failed task gate | do-task (with gate error context) |
| `task_gate` | Verification gate after task implementation | gate (make check) |
| `audit_task` | Independent audit of the most recent task | audit-task |
| `spec_gate` | Full verification gate after all tasks complete | gate (make all) |
| `spec_gate_fix` | Agent fixes spec gate failures | do-task (with spec gate error context) |
| `run_demo` | Execute demo plan and validate | run-demo |
| `demo_retry` | Forceful retry of failed demo | run-demo (with retry context) |
| `audit_spec` | Full-spec audit after demo passes | audit-spec |
| `audit_spec_retry` | Forceful retry of failed spec audit | audit-spec (with retry context) |

### Sub-State Flow Diagram

```
worktree_setup ---> task_select (on success)
       |
       +---> HALT (on error)

                    +---> HALT (limits exceeded)
                    |
task_select --------+---> do_task ---> task_gate ---> audit_task ---+
    ^               |         |            |                        |
    |               |         v            v                        |
    |               |     HALT         do_task_gate_fix             |
    |               |    (blocked/      (gate retry)                |
    |               |     error)           |                        |
    |               |                      +--- (max retries) ---> HALT
    |               |                                               |
    +---------------+-----------------------------------------------+
    |               |
    |               +---> spec_gate ---> run_demo ---> audit_spec ---> COMPLETE
    |                         |              |              |
    |                         v              v              v
    |                    spec_gate_fix   demo_retry   audit_spec_retry
    |                         |              |              |
    +-------------------------+--------------+--------------+
```

### Complete Transition Table

Each row represents: when the workflow monitor processes a result (or
evaluates `task_select`), what transition occurs.

**worktree_setup (after setup result):**

| # | Outcome | Next Sub-State | Action |
|---|---|---|---|
| 0a | outcome=success | `task_select` | Set `worktree_ready=true`. Enter `task_select`. |
| 0b | outcome=error | HALT | `"Worktree creation failed: {tail_output}"` |

**task_select (no dispatch — immediate evaluation):**

| # | Condition | Next Sub-State | Action |
|---|---|---|---|
| 1 | `unchecked_tasks == 0` | `spec_gate` | Dispatch gate (make all) |
| 2 | `cycle > MAX_CYCLES` (10) | HALT | `"Safety limit reached ({MAX_CYCLES} cycles)"` |
| 3 | `staleness.count >= STALE_LIMIT` (3) | HALT | `"Stale — plan.md unchanged for {count} cycles"` |
| 4 | Same task + `task_attempts > MAX_TASK_RETRIES` (5) | HALT | `"Task stuck after {MAX_TASK_RETRIES} attempts: {task}"` |
| 5 | Tasks remain, limits OK | `do_task` | Dispatch do-task for next unchecked task |

On entry to `task_select`:
- Compare `plan_hash` from latest result with `staleness.prev_plan_hash`.
  If unchanged: increment `staleness.count`. If changed: reset to 0.
- Update `staleness.prev_plan_hash` to current `plan_hash`.
- Detect task change: if `current_task` differs from next task label,
  reset `task_attempts` to 0 and `gate_attempt` to 0.
- Increment `task_attempts`.

**do_task (after do-task result):**

| # | Outcome | Signal | Next Sub-State | Action |
|---|---|---|---|---|
| 6 | success | `complete` | `task_gate` | Dispatch gate (make check) |
| 7 | success | `all-done` | `spec_gate` | Dispatch gate (make all) — skip remaining tasks |
| 8 | success | null | `task_gate` | No signal detected; gate will verify |
| 9 | blocked | `blocked` | HALT | `"Human intervention needed. See signposts.md"` |
| 10 | error | `error` | HALT | Include tail_output in halt reason |
| 11 | timeout | null | HALT | `"do-task timed out after {timeout}s"` |
| 12 | success | (other) | `task_gate` | Log warning about unrecognized signal, proceed |

**task_gate (after gate result):**

| # | Outcome | Condition | Next Sub-State | Action |
|---|---|---|---|---|
| 13 | success | — | `audit_task` | Dispatch audit-task |
| 14 | fail | `gate_attempt < MAX_GATE_RETRIES` (3) | `do_task_gate_fix` | Dispatch do-task with gate error context; increment `gate_attempt` |
| 15 | fail | `gate_attempt >= MAX_GATE_RETRIES` (3) | HALT | `"Task gate failing after {MAX_GATE_RETRIES} attempts"`, include gate_output |

**do_task_gate_fix (after do-task-with-gate-errors result):**

| # | Outcome | Signal | Next Sub-State | Action |
|---|---|---|---|---|
| 16 | blocked | `blocked` | HALT | `"do-task blocked while fixing gate"` |
| 17 | error | `error` | HALT | `"do-task error while fixing gate"` |
| 18 | timeout | null | HALT | `"do-task timed out while fixing gate"` |
| 19 | any other | any | `task_gate` | Dispatch gate (make check) — retry gate |

**audit_task (after audit-task result):**

| # | Outcome | Signal | Next Sub-State | Action |
|---|---|---|---|---|
| 20 | success | `pass` | `task_select` | Task clean. Self-heal: if task still unchecked, check it off. |
| 21 | fail | `fail` | `task_select` | Fix items added. Increment `cycle`. |
| 22 | error | `error` | HALT | Include tail_output in halt reason |
| 23 | timeout | null | HALT | `"audit-task timed out"` |
| 24 | success | null | `task_select` | No signal; treat as pass with warning |

**spec_gate (after gate result):**

| # | Outcome | Next Sub-State | Action |
|---|---|---|---|
| 25 | success | `run_demo` | Dispatch run-demo |
| 26 | fail | `spec_gate_fix` | Dispatch do-task with spec gate error context |

**spec_gate_fix (after do-task-with-spec-gate-errors result):**

| # | Outcome | Next Sub-State | Action |
|---|---|---|---|
| 27 | any | `task_select` | Always restart cycle. Increment `cycle`. (Matches orchestrate.sh `continue`.) |

Note: orchestrate.sh does not inspect the do-task signal after spec gate
fix — it unconditionally restarts the cycle. Staleness detection catches
persistent failures.

**run_demo (after run-demo result):**

| # | Outcome | Signal | Condition | Next Sub-State | Action |
|---|---|---|---|---|---|
| 28 | success | `pass` | — | `audit_spec` | Dispatch audit-spec |
| 29 | fail | `fail` | `unchecked_tasks > pre_phase_unchecked` | `task_select` | New tasks added. Increment `cycle`. |
| 30 | fail | `fail` | `unchecked_tasks <= pre_phase_unchecked` | `demo_retry` | No tasks added. Dispatch run-demo with forceful context. |
| 31 | error | `error` | — | HALT | Include tail_output |
| 32 | timeout | null | — | HALT | `"run-demo timed out"` |
| 33 | error | null | no signal + nonzero exit | HALT | `"run-demo failed with no signal"` |

**demo_retry (after forceful run-demo result):**

| # | Outcome | Signal | Condition | Next Sub-State | Action |
|---|---|---|---|---|---|
| 34 | success | `pass` | — | `audit_spec` | Dispatch audit-spec |
| 35 | fail | `fail` | `unchecked_tasks > pre_phase_unchecked` | `task_select` | Tasks added. Increment `cycle`. |
| 36 | fail | `fail` | no new tasks, `demo_retry < MAX` (3) | `demo_retry` | Re-dispatch with forceful context. Increment `demo_retry`. |
| 37 | fail | `fail` | no new tasks, `demo_retry >= MAX` (3) | `task_select` | Retries exhausted. Defer to staleness detection. Increment `cycle`. |

**audit_spec (after audit-spec result):**

| # | Outcome | Signal | Condition | Next Sub-State | Action |
|---|---|---|---|---|---|
| 38 | success | `pass` | — | COMPLETE | Top-level transitions to `await_walk` |
| 39 | fail | `fail` | `unchecked_tasks > pre_phase_unchecked` | `task_select` | Fix items added. Increment `cycle`. |
| 40 | fail | `fail` | `unchecked_tasks <= pre_phase_unchecked` | `audit_spec_retry` | No tasks added. Dispatch audit-spec with forceful context. |
| 41 | error | null | audit.md not written or stale | HALT | `"audit-spec did not create/update audit.md"` |
| 42 | error | null | `status: unknown` in audit.md | HALT | `"audit-spec returned unknown status"` |

**audit_spec_retry (after forceful audit-spec result):**

| # | Outcome | Signal | Condition | Next Sub-State | Action |
|---|---|---|---|---|---|
| 43 | success | `pass` | — | COMPLETE | Top-level transitions to `await_walk` |
| 44 | fail | `fail` | `unchecked_tasks > pre_phase_unchecked` | `task_select` | Fix items added. Increment `cycle`. |
| 45 | fail | `fail` | no new tasks, `audit_retry < MAX` (3) | `audit_spec_retry` | Re-dispatch. Increment `audit_retry`. |
| 46 | fail | `fail` | no new tasks, `audit_retry >= MAX` (3) | `task_select` | Retries exhausted. Defer to staleness detection. Increment `cycle`. |

> **Note on `pre_phase_unchecked`**: The coordinator records
> `unchecked_tasks` from the *previous* result into `pre_phase_unchecked`
> at the time it dispatches run-demo or audit-spec. The comparison
> `unchecked_tasks > pre_phase_unchecked` in the result determines
> whether the phase added new tasks.

### Retry Counter Resets

| Counter | Reset Condition |
|---|---|
| `task_attempts` | `current_task` changes (new task selected) |
| `gate_attempt` | `current_task` changes; also on audit_task entry |
| `demo_retry` | Entering `run_demo` from `spec_gate` (fresh attempt) |
| `audit_retry` | Entering `audit_spec` from `run_demo` (fresh attempt) |
| `staleness.count` | `plan_hash` changes between cycles |
| `cycle` | Never resets within a workflow |

> **Future optimization**: Gate phases (task_gate, spec_gate) could be
> run inline by the worker manager immediately after agent phases
> complete, embedding gate results in the agent result. This would
> eliminate one dispatch per task but adds schema complexity. Deferred —
> the nudge mechanism reduces per-transition latency sufficiently.

---

## 7. Concurrency Model

### Subscription Pools

| Subscription | CLI | Max Concurrent | Phases |
|---|---|---|---|
| Z.ai | opencode | 1 | do-task, run-demo |
| OpenAI Plus | codex | 1 | audit-task, audit-spec |
| none | bash | 3 | gates |

### Rules

1. **Per-subscription serialization**: At most 1 opencode process and 1
   codex process running at any time, across all workflows.

2. **Cross-subscription parallelism**: An opencode worker and a codex
   worker CAN run simultaneously (they consume different subscriptions).

3. **Gate parallelism**: Bash gates run in parallel with everything,
   including each other, up to 3 concurrent gates.

4. **Per-workflow serialization**: Each workflow has at most one
   dispatched phase at a time. The workflow monitor never dispatches a
   second phase for the same workflow until the current one completes.

5. **Max orchestrating workflows**: At most 3 workflows in
   `orchestrating` state simultaneously. New workflows queue at
   `await_confirm` until a slot opens.

### Worker Manager Dispatch Queue

The worker manager maintains three queues:

```
opencode_queue: FIFO  (do-task, run-demo dispatches)
codex_queue:    FIFO  (audit-task, audit-spec dispatches)
gate_queue:     FIFO  (make check, make all dispatches)
```

Processing rules:
- `opencode_queue`: Dequeue and execute when no opencode process is running.
- `codex_queue`: Dequeue and execute when no codex process is running.
- `gate_queue`: Dequeue and execute when fewer than 3 gates are running.
- Check queues on every poll cycle (5s).
- New dispatch files are appended to the appropriate queue on discovery.

### Example: Two Workflows in Parallel

```
Timeline:
  wf-rentl-144: do-task (opencode) ─────────────► gate (bash) ──► audit-task (codex) ──►
  wf-rentl-150: .............. [queued] ......... do-task (opencode) ────────────────────►
                                                  ^ opencode frees up
```

The audit-task for wf-144 (codex) and do-task for wf-150 (opencode) run
in parallel because they use different subscriptions.

---

## 8. Worktree Management

### Naming Convention

```
~/github/{project}-wt-{issue_number}/
```

Examples:
```
~/github/rentl-wt-144/
~/github/rentl-wt-150/
~/github/kaifuu-wt-3/
```

### Lifecycle

The worker manager owns the full worktree lifecycle:

| Event | Action |
|---|---|
| Workflow enters `orchestrating` | Coordinator dispatches setup phase. Worker manager creates worktree from existing branch. |
| Before every dispatch | Validate worktree: correct branch checked out, clean working tree, no merge conflicts. |
| Workflow reaches `completed` or `idle` (cancelled) | Remove worktree: `git worktree remove {path}`. |

### Creation

```bash
cd ~/github/{project}
git worktree add ~/github/{project}-wt-{issue_number} {branch}
```

The branch MUST already exist — shape-spec creates and pushes it before
the workflow reaches `await_confirm`.

### Worktree Registry

The worker manager maintains `worktrees.json` to enforce isolation:

```json
{
  "worktrees": {
    "wf-rentl-144-1741359600": {
      "project": "rentl",
      "issue": 144,
      "branch": "s0146-slug",
      "path": "/home/trevor/github/rentl-wt-144",
      "created_at": "2026-03-07T15:01:00Z"
    }
  }
}
```

**Isolation invariants** (enforced on creation):
- No two workflows share the same branch.
- No two workflows share the same worktree path.
- The worktree path must not be the main working copy.

---

## 9. Timeout and Retry Specifications

| Limit | Value | Source |
|---|---|---|
| Agent timeout | 1800s (30 min) | `ORCH_AGENT_TIMEOUT` |
| Gate timeout | 300s (5 min) | Hardcoded |
| Max cycles | 10 | `ORCH_MAX_CYCLES` |
| Staleness limit | 3 unchanged cycles | `ORCH_STALE_LIMIT` |
| Max task retries | 5 | `ORCH_MAX_TASK_RETRIES` |
| Max gate retries | 3 | Hardcoded (`MAX_GATE_RETRIES`) |
| Max demo retries | 3 | `ORCH_MAX_DEMO_RETRIES` |
| Max audit-spec retries | 3 | `ORCH_MAX_DEMO_RETRIES` (shared constant) |
| Dispatch pickup timeout | 120s | Hardcoded |
| Worker manager poll interval | 5s | `dispatch/` scanning interval |
| Workflow monitor poll interval | 60s | NanoClaw scheduled task cron (fallback; primary path is nudge at ~500ms) |

### Timeout Hierarchy

```
Workflow monitor (60s poll)
  └── checks: dispatched_at + timeout + pickup_grace (120s)
        └── if exceeded and no heartbeat: treat as crash

Worker manager (5s poll)
  └── per-process timeout: dispatch.timeout seconds
        └── SIGTERM -> 5s grace -> SIGKILL (same pattern as orchestrate.sh)
```

---

## 10. Edge Case Handling

### Worker Crash (No Result File)

**Detection**: Workflow monitor checks `dispatched_at + timeout` on every
poll. If exceeded, check for heartbeat.

**Heartbeat mechanism**: Worker manager writes heartbeat files while a
process is running:

```
data/ipc/{coordinator-group}/in-progress/{dispatch-filename-stem}.heartbeat
```

Updated every 30s with current timestamp. Deleted when the process
completes (result file written).

**Recovery**:
- Heartbeat recent (< 60s old): Process still running. Wait.
- Heartbeat stale (> 60s old) or missing, and `dispatched_at + timeout`
  exceeded: Process crashed. Workflow monitor writes a synthetic result
  with `outcome: error`, `signal: null`, `tail_output: "Worker process
  crashed (no result after timeout)"`. State machine processes it
  normally (HALT for most phases).

### Coordinator Restart

**Recovery**: On startup, the coordinator reads `workflows.json` to
reconstruct all workflow states. Then:

1. For each workflow with `dispatched` != null:
   - Check `results/` for a matching result file (by `workflow_id`).
   - If found: process the result, advance state.
   - If not found: check if dispatch file still exists in `dispatch/`.
     If present, the worker manager hasn't picked it up yet — wait.
     If absent, check heartbeat — process may be running.

2. For each workflow in `orchestrating` with `dispatched` == null:
   - Re-enter `task_select` and dispatch the next phase.

### Dispatch Sits Unprocessed

**Detection**: Workflow monitor checks if the dispatch file
(`dispatched.dispatch_file`) still exists in `dispatch/` after 120s.

**Recovery**:
1. First occurrence: Delete the stale dispatch file. Re-dispatch the
   same phase (write a new dispatch file). Post to Discord: "Dispatch
   retry for {workflow_id} — worker manager may be slow."
2. Second occurrence: HALT with `"Worker manager not processing
   dispatches — check worker manager health"`. Post to Discord: "Worker
   manager not processing dispatches for {workflow_id}. Check worker
   health."

### Result Sits Unread

Files persist indefinitely in `results/`. Not a problem — the next 60s
workflow monitor poll picks them up. No TTL or cleanup needed for result
files.

### spec.md Modified by Agent

Handled transparently by the worker manager (see Section 3, signal
extraction item 4). The `spec_modified: true` flag in the result is
informational — the coordinator logs it but doesn't change behavior.

### Stuck Audit Loops

Staleness detection catches this. If `plan_hash` doesn't change across
`STALE_LIMIT` (3) consecutive cycles through `task_select`, the workflow
HALTs with a staleness error. This covers:

- audit-task repeatedly failing on the same issues without adding
  effective fix items
- do-task and audit-task ping-ponging on the same task
- demo/audit-spec retries exhausting without progress

### Concurrent Workflow Slot Exhaustion

If 3 workflows are already orchestrating and a new one reaches
`await_confirm`:
- The human confirmation is accepted.
- State transitions to `orchestrating` but with `sub_state: null` and
  `dispatched: null` (queued).
- Workflow monitor checks for queued workflows on every poll and
  activates them (entering `task_select`) when a slot opens.

---

## 11. Worked Example

Complete trace of a workflow from Discord trigger through PR merge.

**Scenario**: Implement rentl issue #144 with 3 tasks in the plan.

### Step 1: Shape Trigger

Human sends on Discord: `@Aegis shape rentl #144`

Coordinator receives the message via NanoClaw message routing. Starts
shape-spec interactively.

**workflows.json** after state change:
```json
{
  "version": 1,
  "updated_at": "2026-03-07T15:00:00Z",
  "workflows": {
    "wf-rentl-144-1741359600": {
      "project": "rentl",
      "issue": 144,
      "branch": null,
      "spec_folder": null,
      "state": "shaping",
      "sub_state": null,
      "dispatched": null,
      "cycle": 0,
      "retries": { "task_attempts": 0, "gate_attempt": 0, "demo_retry": 0, "audit_retry": 0 },
      "staleness": { "count": 0, "prev_plan_hash": null },
      "worktree_ready": false,
      "current_task": null,
      "pre_phase_unchecked": 0,
      "history": []
    }
  }
}
```

### Step 2: Shape-Spec Completes

Shape-spec creates branch `s0146-slug`, writes spec folder, pushes.

**workflows.json** after state change:
```json
{
  "version": 1,
  "updated_at": "2026-03-07T15:08:00Z",
  "workflows": {
    "wf-rentl-144-1741359600": {
      "project": "rentl",
      "issue": 144,
      "branch": "s0146-slug",
      "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
      "state": "await_confirm",
      "sub_state": null,
      "dispatched": null,
      "cycle": 0,
      "retries": { "task_attempts": 0, "gate_attempt": 0, "demo_retry": 0, "audit_retry": 0 },
      "staleness": { "count": 0, "prev_plan_hash": null },
      "worktree_ready": false,
      "current_task": null,
      "pre_phase_unchecked": 0,
      "history": []
    }
  }
}
```

Coordinator posts to Discord: "Spec shaped for #144. Plan has 3 tasks.
Ready to orchestrate?"

### Step 3: Human Confirms

Human replies: "looks good, go ahead"

Coordinator transitions to orchestrating with `sub_state: "worktree_setup"`.
Dispatches setup phase.

**workflows.json** after state change:
```json
{
  "version": 1,
  "updated_at": "2026-03-07T15:10:00Z",
  "workflows": {
    "wf-rentl-144-1741359600": {
      "project": "rentl",
      "issue": 144,
      "branch": "s0146-slug",
      "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
      "state": "orchestrating",
      "sub_state": "worktree_setup",
      "dispatched": {
        "phase": "setup",
        "dispatched_at": "2026-03-07T15:10:00Z",
        "dispatch_file": "1741360200000-w1t2s3.json"
      },
      "cycle": 0,
      "retries": { "task_attempts": 0, "gate_attempt": 0, "demo_retry": 0, "audit_retry": 0 },
      "staleness": { "count": 0, "prev_plan_hash": null },
      "worktree_ready": false,
      "current_task": null,
      "pre_phase_unchecked": 0,
      "history": []
    }
  }
}
```

**dispatch/1741360200000-w1t2s3.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "setup",
  "project": "rentl",
  "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
  "branch": "s0146-slug",
  "cli": "bash",
  "model": null,
  "gate_cmd": null,
  "context": null,
  "timeout": 300
}
```

Worker manager picks up setup dispatch, creates worktree at
`~/github/rentl-wt-144/`.

**results/1741360205123-s4t5u6.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "setup",
  "outcome": "success",
  "signal": null,
  "exit_code": 0,
  "duration_secs": 5,
  "gate_output": null,
  "tail_output": null,
  "unchecked_tasks": 3,
  "plan_hash": "d4e5f6a7",
  "spec_modified": false
}
```

Transition #0a: setup success -> `task_select`. Set `worktree_ready=true`.

Plan.md has 3 unchecked tasks:
```
- [ ] Task 1: Set up configuration schema
- [ ] Task 2: Implement core pipeline
- [ ] Task 3: Add CLI integration
```

**workflows.json** after entering task_select:
```json
{
  "version": 1,
  "updated_at": "2026-03-07T15:10:05Z",
  "workflows": {
    "wf-rentl-144-1741359600": {
      "project": "rentl",
      "issue": 144,
      "branch": "s0146-slug",
      "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
      "state": "orchestrating",
      "sub_state": "task_select",
      "dispatched": null,
      "cycle": 1,
      "retries": { "task_attempts": 1, "gate_attempt": 0, "demo_retry": 0, "audit_retry": 0 },
      "staleness": { "count": 0, "prev_plan_hash": "d4e5f6a7" },
      "worktree_ready": true,
      "current_task": "Task 1: Set up configuration schema",
      "pre_phase_unchecked": 3,
      "history": [
        { "phase": "setup", "outcome": "success", "signal": null, "duration_secs": 5, "ts": "2026-03-07T15:10:05Z" }
      ]
    }
  }
}
```

### Step 4: Dispatch do-task (Task 1)

`task_select` finds 3 unchecked tasks, no limits exceeded. Transitions
to `do_task`, dispatches do-task.

**dispatch/1741360200123-a3f2b8.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "do-task",
  "project": "rentl",
  "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
  "branch": "s0146-slug",
  "cli": "opencode",
  "model": "glm-5",
  "gate_cmd": null,
  "context": null,
  "timeout": 1800
}
```

**workflows.json** changes:
```json
{
  "sub_state": "do_task",
  "dispatched": {
    "phase": "do-task",
    "dispatched_at": "2026-03-07T15:10:00Z",
    "dispatch_file": "1741360200123-a3f2b8.json"
  }
}
```

Worker manager picks up the dispatch file (deletes it), spawns
`opencode --model glm-5` in the rentl-wt-144 worktree with the do-task
command and spec folder.

### Step 5: do-task Result (Task 1 complete)

Agent implements Task 1, runs make check, commits, signals complete.

**results/1741360542456-c7d91e.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "do-task",
  "outcome": "success",
  "signal": "complete",
  "exit_code": 0,
  "duration_secs": 342,
  "gate_output": null,
  "tail_output": null,
  "unchecked_tasks": 2,
  "plan_hash": "b1c2d3e4",
  "spec_modified": false
}
```

Workflow monitor processes result. Transition #6: success/complete ->
`task_gate`. Dispatches make check.

### Step 6: Dispatch Task Gate

**dispatch/1741360543789-e5f6a7.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "gate",
  "project": "rentl",
  "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
  "branch": "s0146-slug",
  "cli": "bash",
  "model": null,
  "gate_cmd": "make check",
  "context": null,
  "timeout": 300
}
```

### Step 7: Task Gate Result (pass)

**results/1741360630012-f8a9b0.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "gate",
  "outcome": "success",
  "signal": null,
  "exit_code": 0,
  "duration_secs": 87,
  "gate_output": null,
  "tail_output": null,
  "unchecked_tasks": 2,
  "plan_hash": "b1c2d3e4",
  "spec_modified": false
}
```

Transition #13: gate success -> `audit_task`. Dispatch audit-task.

### Step 8: Dispatch audit-task (Task 1)

**dispatch/1741360631345-c2d3e4.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "audit-task",
  "project": "rentl",
  "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
  "branch": "s0146-slug",
  "cli": "codex",
  "model": "gpt-5.3-codex",
  "gate_cmd": null,
  "context": null,
  "timeout": 1800
}
```

### Step 9: audit-task Result (pass)

**results/1741360781678-d4e5f6.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "audit-task",
  "outcome": "success",
  "signal": "pass",
  "exit_code": 0,
  "duration_secs": 150,
  "gate_output": null,
  "tail_output": null,
  "unchecked_tasks": 2,
  "plan_hash": "b1c2d3e4",
  "spec_modified": false
}
```

Transition #20: pass -> `task_select`.

**workflows.json** after processing:
```json
{
  "sub_state": "task_select",
  "dispatched": null,
  "current_task": "Task 1: Set up configuration schema",
  "history": [
    { "phase": "setup", "outcome": "success", "signal": null, "duration_secs": 5, "ts": "2026-03-07T15:10:05Z" },
    { "phase": "do-task", "outcome": "success", "signal": "complete", "duration_secs": 342, "ts": "2026-03-07T15:15:42Z" },
    { "phase": "gate", "outcome": "success", "signal": null, "duration_secs": 87, "ts": "2026-03-07T15:17:10Z" },
    { "phase": "audit-task", "outcome": "success", "signal": "pass", "duration_secs": 150, "ts": "2026-03-07T15:19:41Z" }
  ]
}
```

### Steps 10-17: Tasks 2 and 3 (Same Pattern)

Each task follows the same cycle: `task_select` -> `do_task` ->
`task_gate` -> `audit_task` -> `task_select`.

After Task 3 completes and audit passes, `unchecked_tasks` drops to 0.

### Step 18: task_select -> spec_gate

`task_select` finds 0 unchecked tasks. Transition #1: -> `spec_gate`.

**dispatch/1741363200123-g7h8i9.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "gate",
  "project": "rentl",
  "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
  "branch": "s0146-slug",
  "cli": "bash",
  "model": null,
  "gate_cmd": "make all",
  "context": null,
  "timeout": 300
}
```

### Step 19: Spec Gate Result (pass)

**results/1741363320456-j0k1l2.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "gate",
  "outcome": "success",
  "signal": null,
  "exit_code": 0,
  "duration_secs": 120,
  "gate_output": null,
  "tail_output": null,
  "unchecked_tasks": 0,
  "plan_hash": "f5a6b7c8",
  "spec_modified": false
}
```

Transition #25: spec_gate success -> `run_demo`. Dispatch run-demo.

### Step 20: Dispatch run-demo

**dispatch/1741363321789-m3n4o5.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "run-demo",
  "project": "rentl",
  "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
  "branch": "s0146-slug",
  "cli": "opencode",
  "model": "glm-5",
  "gate_cmd": null,
  "context": null,
  "timeout": 1800
}
```

### Step 21: run-demo Result (pass)

**results/1741363921012-p6q7r8.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "run-demo",
  "outcome": "success",
  "signal": "pass",
  "exit_code": 0,
  "duration_secs": 600,
  "gate_output": null,
  "tail_output": null,
  "unchecked_tasks": 0,
  "plan_hash": "f5a6b7c8",
  "spec_modified": false
}
```

Transition #28: pass -> `audit_spec`. Dispatch audit-spec.

### Step 22: Dispatch audit-spec

**dispatch/1741363922345-s9t0u1.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "audit-spec",
  "project": "rentl",
  "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
  "branch": "s0146-slug",
  "cli": "codex",
  "model": "gpt-5.3-codex",
  "gate_cmd": null,
  "context": null,
  "timeout": 1800
}
```

### Step 23: audit-spec Result (pass)

**results/1741364522678-v2w3x4.json:**
```json
{
  "workflow_id": "wf-rentl-144-1741359600",
  "phase": "audit-spec",
  "outcome": "success",
  "signal": "pass",
  "exit_code": 0,
  "duration_secs": 600,
  "gate_output": null,
  "tail_output": null,
  "unchecked_tasks": 0,
  "plan_hash": "f5a6b7c8",
  "spec_modified": false
}
```

Transition #38: audit_spec pass -> COMPLETE. Top-level transitions to
`await_walk`.

**workflows.json** after COMPLETE:
```json
{
  "version": 1,
  "updated_at": "2026-03-07T16:02:02Z",
  "workflows": {
    "wf-rentl-144-1741359600": {
      "project": "rentl",
      "issue": 144,
      "branch": "s0146-slug",
      "spec_folder": "tanren/specs/2026-02-19-1531-s0146-slug",
      "state": "await_walk",
      "sub_state": null,
      "dispatched": null,
      "cycle": 1,
      "retries": { "task_attempts": 1, "gate_attempt": 0, "demo_retry": 0, "audit_retry": 0 },
      "staleness": { "count": 0, "prev_plan_hash": "f5a6b7c8" },
      "worktree_ready": true,
      "current_task": null,
      "pre_phase_unchecked": 0,
      "history": [
        { "phase": "setup", "outcome": "success", "signal": null, "duration_secs": 5, "ts": "2026-03-07T15:10:05Z" },
        { "phase": "do-task", "outcome": "success", "signal": "complete", "duration_secs": 342, "ts": "2026-03-07T15:15:42Z" },
        { "phase": "gate", "outcome": "success", "signal": null, "duration_secs": 87, "ts": "2026-03-07T15:17:10Z" },
        { "phase": "audit-task", "outcome": "success", "signal": "pass", "duration_secs": 150, "ts": "2026-03-07T15:19:41Z" },
        { "phase": "do-task", "outcome": "success", "signal": "complete", "duration_secs": 280, "ts": "2026-03-07T15:24:21Z" },
        { "phase": "gate", "outcome": "success", "signal": null, "duration_secs": 90, "ts": "2026-03-07T15:25:51Z" },
        { "phase": "audit-task", "outcome": "success", "signal": "pass", "duration_secs": 130, "ts": "2026-03-07T15:28:01Z" },
        { "phase": "do-task", "outcome": "success", "signal": "complete", "duration_secs": 410, "ts": "2026-03-07T15:34:51Z" },
        { "phase": "gate", "outcome": "success", "signal": null, "duration_secs": 85, "ts": "2026-03-07T15:36:16Z" },
        { "phase": "audit-task", "outcome": "success", "signal": "pass", "duration_secs": 140, "ts": "2026-03-07T15:38:36Z" },
        { "phase": "gate", "outcome": "success", "signal": null, "duration_secs": 120, "ts": "2026-03-07T15:40:36Z" },
        { "phase": "run-demo", "outcome": "success", "signal": "pass", "duration_secs": 600, "ts": "2026-03-07T15:50:36Z" },
        { "phase": "audit-spec", "outcome": "success", "signal": "pass", "duration_secs": 600, "ts": "2026-03-07T16:00:36Z" }
      ]
    }
  }
}
```

Coordinator posts to Discord: "Orchestration complete for #144
(3 tasks, 1 cycle, 13 phases, 48m 41s). Say 'walk' to start walk-spec
or 'defer' to do it later."

### Step 24: Human Triggers Walk

Human replies: "walk"

State: `await_walk` -> `walking`

### Step 25: Walk-Spec

Coordinator runs walk-spec interactively over Discord. Human reviews
demo, checks implementation, approves.

State: `walking` -> `pr_review`

### Step 26: PR Created and Merged

Coordinator creates PR via GitHub CLI. Human reviews and merges.

State: `pr_review` -> `completed`

Worker manager cleans up worktree:
```bash
git worktree remove ~/github/rentl-wt-144
```

Workflow archived in `workflows.json` with `state: completed`.

---

## Appendix A: Signal Reference

Signals defined in each tanren command file:

| Command | Signal Prefix | Values |
|---|---|---|
| do-task | `do-task-status` | `complete`, `blocked`, `all-done`, `error` |
| audit-task | `audit-task-status` | `pass`, `fail`, `error` |
| run-demo | `run-demo-status` | `pass`, `fail`, `error` |
| audit-spec | (audit.md `status:` line) | `pass`, `fail` |

## Appendix B: orchestrate.sh Line Reference

Cross-reference from protocol sub-states to orchestrate.sh source:

| Sub-State | orchestrate.sh Lines | Description |
|---|---|---|
| task_select | 485-549 | Cycle loop, staleness, task counting, retry tracking |
| do_task | 555-588 | do-task invocation and signal handling |
| task_gate | 594-598 | Gate execution |
| do_task_gate_fix | 607-623 | Re-invoke do-task with gate errors |
| audit_task | 627-674 | audit-task invocation, self-heal checkbox |
| spec_gate | 679 | `run_gate "make all"` |
| spec_gate_fix | 681-691 | do-task with spec gate errors, `continue` |
| run_demo | 697-799 | run-demo invocation, retry loop |
| demo_retry | 735-782 | Forceful retry with demand for tasks |
| audit_spec | 810-903 | audit-spec invocation, audit.md validation, retry loop |
| audit_spec_retry | 851-896 | Forceful retry with demand for fix items |
