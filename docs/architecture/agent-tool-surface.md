# Agent Tool Surface

Authoritative spec for the typed tool surface that agents use to
interact with the Tanren orchestrator. Every structured state
mutation an agent performs — creating tasks, recording findings,
setting evidence frontmatter, signalling phase outcomes — happens
through one of these tools.

Companion docs:
[orchestration-flow.md](orchestration-flow.md),
[evidence-schemas.md](evidence-schemas.md),
[audit-rubric.md](audit-rubric.md),
[adherence.md](adherence.md).

---

## 1. Principles

1. **No raw file writes of structured content.** Agents never hand-
   author frontmatter, task lists, finding lists, rubric scores, or
   event files. Every such state transits a typed tool.
2. **Schema enforcement at the tool boundary.** Every tool validates
   its input against a serde schema; invalid input returns a typed
   `ToolError { field_path, expected, actual, remediation }`.
3. **Per-phase capability scopes.** Each phase receives a typed
   capability set at dispatch time; tools outside that set return
   `CapabilityDenied`.
4. **Transport-agnostic contract.** Two transports (MCP, CLI) share
   one service. Schema, side-effects, and errors are identical.
5. **Immutable event log.** Every emitted methodology event appends one
   typed line to `{spec_folder}/phase-events.jsonl` (committed artifact)
   and applies it to the store. A single tool call may emit multiple
   events when semantics require it; derived events carry explicit
   causal linkage.

---

## 2. Transports

### 2.1 MCP (primary)

- Binary: `tanren-mcp`.
- SDK: [`rmcp`](https://crates.io/crates/rmcp) from
  `modelcontextprotocol/rust-sdk`. Features: `server`,
  `transport-io`, `macros`.
- Runtime: tokio.
- Transport: stdio only (Lane 0.5). SSE/HTTP deferred.
- Tool registration: a compile-time typed registry binds each tool's
  descriptor, JSON schema builder, and dispatch target in one table.
  JSON Schema is still derived from Rust contract types.
- Logging: **stderr only**. Writing to stdout corrupts JSON-RPC
  framing. Workspace lints already forbid `println!`/`eprintln!`/
  `dbg!`; use:
  ```rust
  tracing_subscriber::fmt()
      .with_writer(std::io::stderr)
      .init();
  ```

### 2.2 CLI fallback

- Binary: `tanren-cli`.
- Subcommands mirror every tool 1:1:
  ```
  tanren-cli --database-url <db_url> methodology \
    --phase <phase> \
    --spec-id <spec_uuid> \
    --spec-folder <spec_dir> \
    <noun> <verb> \
    --params-file <payload.json>

  # Other input forms:
  tanren-cli ... --json '<payload>'
  tanren-cli ... --params-stdin < payload.json
  ```
- Used when MCP isn't wired (e.g., during early self-hosting, CI
  scripts, or agents without MCP support).

### 2.3 Transport parity

Both transports call the same `tanren-app-services::methodology::
service` methods. Transport parity is event-for-event and
tool-contract-for-tool-contract. `tanren replay <spec_folder>`
reconstructs identical store state from the JSONL.

---

## 3. Tool catalog (by capability group)

Every tool's full typed schema lives in
`crates/tanren-contract/src/methodology/` as Rust types with serde +
schemars derives. The summary below is canonical intent; the Rust
types are canonical syntax.

### 3.1 Core task ops

| Tool | Capability | Purpose |
|---|---|---|
| `create_task(title, description, parent_task_id?, depends_on?, origin, acceptance_criteria[])` → `TaskId` | `task.create` | Materialize a pending task. |
| `start_task(task_id)` | `task.start` | `Pending → InProgress`. Usually called implicitly at session start for do-task. |
| `complete_task(task_id, evidence_refs)` | `task.complete` | `InProgress → Implemented`. |
| `mark_task_guard_satisfied(task_id, guard, idempotency_key?)` | `task.complete` | Records one guard pass (`gate_checked`, `audited`, `adherent`, or extra guard) and emits `TaskCompleted` when required guards converge. |
| `revise_task(task_id, revised_description, revised_acceptance, reason)` | `task.revise` | Mutate non-terminal task scope; emits `TaskRevised`. |
| `abandon_task(task_id, reason, disposition, replacements[], explicit_user_discard_provenance?)` | `task.abandon` | Branch to `Abandoned` with typed `replacement` vs `explicit_user_discard` semantics. |
| `list_tasks(filter?)` → `[Task]` | `task.read` | Query tasks for `filter.spec_id` when supplied; otherwise query the current bound session spec. |

### 3.2 Findings and rubric

| Tool | Capability | Purpose |
|---|---|---|
| `add_finding(severity, title, description, affected_files, line_numbers?, source_phase, attached_task?, pillar?, standard_ref?)` → `FindingId` | `finding.add` | Typed finding. Severity ∈ `{fix_now, defer, note, question}`. |
| `record_rubric_score(pillar, score, rationale, supporting_finding_ids[])` | `rubric.record` | Score 1–10. Validates finding linkage: `score < pillar.target` → non-empty findings; `score < pillar.passing` → at least one `fix_now`. |
| `record_non_negotiable_compliance(name, status, rationale)` | `compliance.record` | Typed pass/fail compliance. |

### 3.3 Spec frontmatter

| Tool | Capability |
|---|---|
| `set_spec_title(title)` | `spec.frontmatter` |
| `set_spec_problem_statement(problem_statement)` | `spec.frontmatter` |
| `set_spec_motivations(motivations[])` | `spec.frontmatter` |
| `set_spec_expectations(expectations[])` | `spec.frontmatter` |
| `set_spec_planned_behaviors(planned_behaviors[])` | `spec.frontmatter` |
| `set_spec_implementation_plan(implementation_plan[])` | `spec.frontmatter` |
| `set_spec_non_negotiables(items[])` | `spec.frontmatter` |
| `add_spec_acceptance_criterion(id, description, measurable)` | `spec.frontmatter` |
| `set_spec_demo_environment(connections[])` | `spec.frontmatter` |
| `set_spec_dependencies(depends_on_spec_ids[], external_issue_refs[])` | `spec.frontmatter` |
| `set_spec_base_branch(branch)` | `spec.frontmatter` |
| `set_spec_relevance_context(touched_files[], project_language?, tags[], category?)` | `spec.frontmatter` |

### 3.4 Demo frontmatter

| Tool | Capability |
|---|---|
| `add_demo_step(id, mode, description, expected_observable)` | `demo.frontmatter` |
| `mark_demo_step_skip(id, reason)` | `demo.frontmatter` |
| `append_demo_result(step_id, status, observed)` | `demo.results` |

### 3.5 Signposts

| Tool | Capability |
|---|---|
| `add_signpost(task_id?, status, problem, evidence, tried[], files_affected[])` → `SignpostId` | `signpost.add` |
| `update_signpost_status(id, status, resolution?)` | `signpost.update` |

### 3.6 Phase lifecycle

| Tool | Capability | Restriction |
|---|---|---|
| `report_phase_outcome(spec_id, task_id?, outcome)` | `phase.outcome` | All agentic phases; `task_id` required for task-scoped `audit-task` / `adhere-task` completion bridging. |
| `escalate_to_blocker(reason, options[])` | `phase.escalate` | **`investigate` only.** |
| `post_reply_directive(thread_ref, body, disposition)` | `feedback.reply` | **`handle-feedback` only.** |

### 3.7 Backlog

| Tool | Capability | Users |
|---|---|---|
| `create_issue(title, description, suggested_spec_scope, priority)` → `IssueRef` | `issue.create` | `triage-audits`, `handle-feedback(out-of-scope)` |

### 3.8 Standards and adherence

| Tool | Capability | Users |
|---|---|---|
| `list_relevant_standards(spec_id, touched_files?, project_language?, domains?, tags?, category?)` → `[RelevantStandard]` | `standard.read` | adherence phases |
| `record_adherence_finding(standard {category,name}, affected_files, line_numbers?, severity, rationale)` → `FindingId` | `adherence.record` | adherence phases |

---

## 4. Per-phase capability scopes

Enforced at dispatch time:
- CLI reads `TANREN_PHASE_CAPABILITIES` (comma-separated capability tags).
- MCP requires a signed `TANREN_MCP_CAPABILITY_ENVELOPE` claim set
  (issuer/audience/exp/nbf/iat/jti/phase/spec/session/capabilities)
  verified against an Ed25519 public key at server startup.

Out-of-scope calls return `CapabilityDenied`. Unknown capability tags
are validation failures (hard error), not silently ignored.

| Phase | Capabilities |
|---|---|
| `shape-spec` | task.create, task.revise, task.read, spec.frontmatter, demo.frontmatter, signpost.add, phase.outcome |
| `do-task` | task.start, task.complete, signpost.add, signpost.update, task.read, phase.outcome |
| `audit-task` | finding.add, rubric.record, compliance.record, task.read, phase.outcome |
| `adhere-task` | standard.read, adherence.record, task.read, phase.outcome |
| `run-demo` | demo.results, finding.add, signpost.add, task.read, phase.outcome |
| `audit-spec` | finding.add, rubric.record, compliance.record, task.read, phase.outcome |
| `adhere-spec` | standard.read, adherence.record, task.read, phase.outcome |
| `walk-spec` | task.create, task.read, phase.outcome |
| `handle-feedback` | task.create, issue.create, feedback.reply, task.read, phase.outcome |
| `investigate` | task.create, task.revise, task.abandon, finding.add, phase.escalate, task.read, phase.outcome |
| `resolve-blockers` | task.create, task.revise, task.abandon, task.read, phase.outcome |
| `triage-audits` | issue.create, finding.add, phase.outcome |
| `sync-roadmap` | finding.add, phase.outcome |
| `discover-standards` | standard.read, phase.outcome |
| `index-standards` | standard.read, phase.outcome |
| `inject-standards` | standard.read, phase.outcome |
| `plan-product` | phase.outcome |

Other combinations denied.

---

## 5. Schema validation contract

Every tool validates input before any side effect. On failure:

```rust
pub enum ToolError {
    ValidationFailed {
        field_path: String,   // JSON-pointer, e.g. "/acceptance_criteria/0/id"
        expected: String,     // e.g. "non-empty string matching /[a-z0-9-]+/"
        actual: String,       // elided for secrets
        remediation: String,  // e.g. "Pass a non-empty identifier"
    },
    CapabilityDenied { capability: String, phase: String },
    IllegalTaskTransition { task_id: TaskId, from: TaskStatus, attempted: String },
    RubricInvariantViolated { pillar: String, score: u8, reason: String },
    Conflict { resource: String, reason: String },
    NotFound { resource: String, key: String },
    Internal { reason: String },
}
```

All errors are typed, serde-serializable, and round-trip through MCP
without loss.

---

## 6. Event log contract

### 6.1 File

`{spec_folder}/phase-events.jsonl` — append-only, committed, one
event per line.

### 6.2 Event envelope

```jsonc
{
  "schema_version": "1.0.0",
  "event_id": "<uuid-v7>",
  "spec_id": "<uuid-v7>",
  "phase": "do-task",
  "agent_session_id": "<string>",
  "timestamp": "<iso-8601>",
  "caused_by_tool_call_id": "<string|null>",
  "origin_kind": "tool_primary|tool_derived|system",
  "tool": "create_task",
  "payload": { /* tool-specific typed payload */ }
}
```

`tool` records the canonical emitting tool name for every line.

### 6.3 Tool-Call Causality

- Primary tool emissions set `origin_kind = tool_primary`.
- Derived emissions from the same tool call set
  `origin_kind = tool_derived` and must include
  `caused_by_tool_call_id`.
- System/postflight emissions set `origin_kind = system`.
- Multi-event calls (for example guard mark + task completion) must
  share the same `caused_by_tool_call_id`.
### 6.4 Atomicity

Service writes event + phase-event-outbox rows in one DB transaction.
`phase-events.jsonl` projection performs budgeted best-effort draining in
the mutation path, plus periodic retry-worker reconciliation from the same
durable outbox rows. Mutation success depends on durable event append; it
is not coupled to full outbox exhaustion in the same call.

### 6.5 Replay

`tanren replay <spec_folder>` reads the JSONL, re-applies each event
to the store via a validated replay-apply path
(`tanren-store::methodology::replay::ingest_phase_events`) that
checks canonical envelope shape, tool/payload consistency, task
transition legality, required provenance metadata (`origin_kind`,
`caused_by_tool_call_id` for `tool_derived`), and event-id
idempotency. Replay is strict: missing provenance metadata is rejected.
Produces the same projections the live run would. Used for recovery
and debugging.

---

## 7. Tool enforcement semantics

### 7.1 Missing tool calls

| Scenario | Orchestrator behavior |
|---|---|
| Process exit 0 + `report_phase_outcome(complete)` called | Record outcome; advance state per phase semantics. |
| Process exit 0 + no `report_phase_outcome` call | Lenient default: `Implemented` (for do-task) or `Complete` (for other agentic phases), conditional on downstream gate/audit agreement. |
| Process exit 0 + `report_phase_outcome(blocked)` | Route to `investigate`. |
| Process exit 0 + `report_phase_outcome(error)` | Route to `investigate`. |
| Process exit non-zero | Record `Error`; route to `investigate` regardless of tool calls. |
| Process timeout | Record `Timeout`; classify as transient; retry ≤ 3 with fresh session. |

### 7.2 Conflicting signals

| Scenario | Behavior |
|---|---|
| Agent calls `complete_task(t)` but downstream gate fails | Task advances to `Implemented`; `GateChecked` guard remains unsatisfied; orchestrator routes to investigate. |
| Agent calls `complete_task(t)` then process exit non-zero | `Error`; orchestrator routes to investigate; task stays at `InProgress`. |
| Agent calls `escalate_to_blocker` from non-investigate phase | `CapabilityDenied`; agent session receives the typed error. |

### 7.3 Idempotence

All tools are idempotent on re-call with identical content (same
task_id + same fields = no new event, no state change). Prevents
duplicate events from network-level retries.

---

## 8. Standard vs domain types

- **Event types** live in `tanren-domain::methodology::events`. They
  are the canonical typed history.
- **Tool input/output types** live in `tanren-contract::methodology`.
  They are the canonical wire schemas, stable across transport and
  version.
- **Projections** live in `tanren-store::methodology`. They are the
  query surfaces the service uses to resolve commands.

The service translates tool-input → validated domain event + store
mutation atomically, then projects `phase-events.jsonl` via a durable
outbox worker and materializes the generated artifact set
(`spec.md`, `plan.md`, `tasks.md`, `tasks.json`, `demo.md`,
`progress.json`, manifest). Processed-row projection I/O failures
return typed errors, while backlog draining remains best-effort and is
recovered by reconciliation without losing canonical events.

---

## 9. Versioning

- MCP protocol version: negotiated per session via `rmcp` handshake;
  server advertises the highest supported revision.
- Tool schema version: `tanren.methodology.v1`. Every tool payload
  carries `schema_version` and MCP `_meta.schema_version` mirrors the
  same value. Backward-compatible additions are minor bumps (new
  optional fields); unknown input fields are rejected at the boundary.
  Breaking changes bump major.
- Event schema version: event envelope `schema_version` is authoritative.

---

## 10. See also

- Orchestration state machine: [orchestration-flow.md](orchestration-flow.md)
- Evidence document schemas:
  [evidence-schemas.md](evidence-schemas.md)
- Audit rubric semantics: [audit-rubric.md](audit-rubric.md)
- Adherence semantics: [adherence.md](adherence.md)
- Install targets and MCP config generation:
  [install-targets.md](install-targets.md)
- Design rationale: [../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md](../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md)
