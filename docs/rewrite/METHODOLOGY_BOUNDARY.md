# Tanren Clean-Room Rewrite: Methodology Boundary

## Purpose

This document defines the boundary between **tanren-code** and
**tanren-markdown** for Tanren 2.0.

The rule:

- **tanren-code** owns workflow mechanics, typed orchestration state,
  and repo-specific resolution.
- **tanren-markdown** owns agent role instructions. Nothing more.

See also:
[LANE-0.5-DESIGN-NOTES.md](tasks/LANE-0.5-DESIGN-NOTES.md) (rationale),
[architecture/orchestration-flow.md](../architecture/orchestration-flow.md),
[architecture/agent-tool-surface.md](../architecture/agent-tool-surface.md),
[architecture/evidence-schemas.md](../architecture/evidence-schemas.md),
[architecture/audit-rubric.md](../architecture/audit-rubric.md),
[architecture/adherence.md](../architecture/adherence.md),
[architecture/install-targets.md](../architecture/install-targets.md),
[architecture/phase-taxonomy.md](../architecture/phase-taxonomy.md).

---

## Operational ownership table

Every workflow concern is owned by `tanren-code`. Rationale in the
final column.

| Concern | Owner | Rationale |
|---|---|---|
| Issue fetch / create / update | tanren-code | owns the provider adapter and credentials; the agent prompt surface is read-only for provider state |
| Candidate task selection | tanren-code | owns the workflow state and dependency graph; selection is a deterministic projection, not an agent choice |
| Branch prep, commit, push, PR creation | tanren-code | owns SCM mechanics so every action is deterministic and auditable |
| Verification hook resolution | tanren-code | owns the command/phase-keyed priority chain |
| Gate execution (TASK_GATE, SPEC_GATE) | tanren-code | owns automated gate invocation end-to-end; agents never run gates |
| Task state transitions | tanren-code | owns the typed state machine and guard evaluation |
| Finding → new-task materialization | tanren-code | owns remediation dispatch; `Complete` is terminal, so every remediation is a fresh task |
| Evidence frontmatter management | tanren-code (via tools) | owns schema validation at the tool boundary; agents only call typed tools |
| generated artifact rendering (`spec.md`, `plan.md`, `tasks.md`, `tasks.json`, `demo.md`, `progress.json`) | tanren-code | owns the event-projected artifact suite with three-layer read-only enforcement |
| Template variable rendering | tanren-code | owns install-time rendering per target |
| Install-target format driver dispatch | tanren-code | owns the Claude Code / Codex Skills / OpenCode / standards-baseline drivers |
| MCP server registration | tanren-code | owns the typed tool catalog and stdio transport |
| Review-comment reply dispatch | tanren-code | owns the provider-adapter reply path; agents emit typed directives only |
| Escalation to blocker | tanren-code | owns the escalation channel; capability is scoped to `investigate` |
| Cross-spec intent/merge conflict events | tanren-code | owns typed event emission now; the resolution engine lands in Phase 2+ |

Everything else — the *how* of agent behavior within its role — is
owned by tanren-markdown.

---

## Tanren-markdown responsibilities

Shared command markdown owns:

- what role the agent is performing
- what inputs it consumes from its dispatch context
- what verification hook (via template variable) it should invoke
- what tools (via the typed tool surface) it must call to record
  outputs
- what narrative evidence (markdown body) it authors
- what's out of its scope

Shared command markdown **must not** hardcode:

- literal verification commands (use `{{TASK_VERIFICATION_HOOK}}`,
  `{{SPEC_VERIFICATION_HOOK}}`, or phase-specific variants)
- issue tracker shell commands (`gh issue …`, `linear issue …`, etc.)
- branch creation or checkout steps
- commit / push / PR steps
- workflow-target selection logic ("find the next task")
- direct writes to orchestrator-owned artifacts (`spec.md`, `plan.md`,
  `tasks.md`, `tasks.json`, `demo.md`, `audit.md`, `signposts.md`,
  `progress.json`, `.tanren-projection-checkpoint.json`) or any
  orchestrator-owned artifact

Shared command markdown **must not** construct structured state
(tasks, findings, rubric scores, evidence frontmatter) by writing
files directly. Every structured mutation goes through the typed tool
surface.

---

## Template variables

`{{DOUBLE_BRACE_UPPER}}` placeholders are filled install-time per
repo profile. Full taxonomy in
[architecture/install-targets.md](../architecture/install-targets.md).
Key ones:

- `{{TASK_VERIFICATION_HOOK}}` / `{{SPEC_VERIFICATION_HOOK}}` +
  per-phase overrides
- `{{ISSUE_PROVIDER}}`, `{{ISSUE_REF_NOUN}}`, `{{PR_NOUN}}`
- `{{SPEC_ROOT}}`, `{{PRODUCT_ROOT}}`, `{{STANDARDS_ROOT}}`
- `{{PROJECT_LANGUAGE}}`
- `{{TASK_TOOL_BINDING}}` (binding-specific actionable invocation prose:
  MCP-primary instructions plus canonical `tanren-cli methodology ...` fallback
  with required globals)
- `{{READONLY_ARTIFACT_BANNER}}` (three-layer read-only warning)
- `{{PILLAR_LIST}}`, `{{REQUIRED_GUARDS}}` (for audit command prose)

Unknown variables in a template → install-time hard error. Variables
declared in a command's frontmatter but never referenced → hard
error. Variables referenced but not declared → hard error.

---

## Agent tool surface

Every structured state mutation happens via a typed tool (MCP or CLI
fallback). Full catalog in
[architecture/agent-tool-surface.md](../architecture/agent-tool-surface.md).
Groups:

- **Task ops** — `create_task`, `start_task`, `complete_task`,
  `revise_task`, `abandon_task`, `list_tasks`
- **Findings + rubric** — `add_finding`, `record_rubric_score`,
  `record_non_negotiable_compliance`
- **Spec / demo frontmatter** — `set_spec_*`, `add_spec_*`,
  `add_demo_step`, `mark_demo_step_skip`, `append_demo_result`
- **Signposts** — `add_signpost`, `update_signpost_status`
- **Phase lifecycle** — `report_phase_outcome`, `escalate_to_blocker`
  (investigate only), `post_reply_directive` (handle-feedback only)
- **Backlog** — `create_issue`
- **Adherence** — `list_relevant_standards`,
  `record_adherence_finding`

Tools are phase-capability-scoped; out-of-scope calls return
`CapabilityDenied`.

---

## Task monotonicity

- `Complete` is terminal.
- Every remediation is a new task with typed `TaskOrigin`.
- `plan.md`, `tasks.md`, `tasks.json`, and `progress.json` are
  generated views over typed event projections; agents never edit
  them.
- `Abandoned` requires replacement tasks or explicit user discard.

Multi-guard completion: a task transitions to `Complete` only when
all required forward guards are satisfied. Default set:
`[gate_checked, audited, adherent]`. Configurable in `tanren.yml`
`methodology.task_complete_requires`. Guards execute independently
and can run in parallel.

---

## Command-level split

### `shape-spec`

- **Owns:** collaborative scope, non-negotiables, acceptance
  criteria, demo plan, initial task breakdown.
- **Does not own:** issue fetch/create, candidate selection,
  dependency mutation, branch prep, issue-body updates, task
  materialization shape (uses `create_task` tool, not markdown
  edits).

### `do-task`

- **Owns:** implementing the supplied task.
- **Does not own:** task selection, gate execution, commit/push/PR,
  task completion state (calls `complete_task`; orchestrator records
  `Implemented`).

### `audit-task`

- **Owns:** applying the 10-pillar rubric to the supplied task/diff;
  producing typed findings; emitting rubric scores.
- **Does not own:** fix-item insertion into projected plan/tasks
  artifacts, task
  creation/un-checking. Fix_now findings materialize new tasks via
  orchestrator.

### `adhere-task`

- **Owns:** filtering to relevant standards for the spec; checking
  the task's diff; emitting adherence findings.
- **Does not own:** rubric scoring, task creation. Relevant standards
  list is provided by `list_relevant_standards`.

### `run-demo`

- **Owns:** executing [RUN] steps, recording results, emitting
  findings per failure.
- **Does not own:** routing failures into workflow state, creating
  new tasks.

### `audit-spec`

- **Owns:** whole-spec 10-pillar rubric audit, non-negotiable
  compliance recording, fix-now vs defer classification.
- **Does not own:** deferred-issue creation (orchestrator via
  `create_issue`), roadmap mutation.

### `adhere-spec`

- **Owns:** spec-level standards compliance across accumulated diff.
- **Does not own:** rubric scoring, task creation.

### `walk-spec`

- **Owns:** human validation walkthrough, acceptance confirmation.
- **Does not own:** PR creation, roadmap update, issue-comment
  posting. Orchestrator performs all of these on accept.

### `handle-feedback`

- **Owns:** classifying review items; emitting reply directives and
  `create_task` / `create_issue` calls.
- **Does not own:** direct `gh api` calls; commit/push; PR merging.

### `investigate`

- **Owns:** root-cause analysis; typed output via `revise_task`,
  `create_task`, `add_finding`, or `escalate_to_blocker`.
- **Does not own:** implementing fixes (emits tasks for `do-task` to
  pick up); is the sole caller of `escalate_to_blocker`.

### `resolve-blockers`

- **Owns:** interactive presentation of investigation-derived
  options; applying the user's choice via typed tools.
- **Does not own:** cascading escalation (no call to
  `escalate_to_blocker`).

### Project-management commands
`sync-roadmap`, `triage-audits`, `discover-standards`,
`index-standards`, `inject-standards`, `plan-product` live under
`commands/project/` and are **not** part of the spec-orchestration
state machine. Each declares its own autonomy. `triage-audits` emits
**issues** (backlog) via `create_issue`, never tasks.

---

## Artifact policy

Fixed Tanren structure:
- `tanren/specs`, `tanren/product`, `tanren/standards`

Agent-authored narrative (markdown body):
- `audit.md`, `signposts.md`

Tool-authored structured frontmatter (same files):
- `SpecFrontmatter`, `DemoFrontmatter`, `AuditFrontmatter`,
  `SignpostsFrontmatter` — schemas in
  [evidence-schemas.md](../architecture/evidence-schemas.md)

Orchestrator-owned (read-only to agents, three-layer enforced):
- `spec.md`, `plan.md`, `tasks.md`, `tasks.json`, `demo.md`,
  `progress.json`, `.tanren-generated-artifacts.json`,
  `phase-events.jsonl` (append-only via tools)

Committed audit trail:
- `phase-events.jsonl` per spec folder — one typed event per tool
  call; replayable.

---

## Manual self-hosting

Before Phase 1 automation, the 7-step manual sequence demonstrates
the methodology end-to-end:

1. User invokes `shape-spec` (interactive).
2. Orchestrator resolves task context for task 1.
3. User invokes `do-task` with the supplied task id.
4. User invokes `audit-task` with the supplied task id + diff.
5. User invokes `run-demo` with the supplied demo context.
6. User invokes `audit-spec` with the supplied spec target.
7. User invokes `walk-spec` (interactive).

Every step's structured output flows through typed tools. The
orchestrator maintains task state, materializes new tasks from
findings, and surfaces progress via generated projections (`plan.md`,
`tasks.md`, `tasks.json`, `progress.json`).

---

## Phase 0 implication

Lane `0.4` (merged) — Rust dispatch CRUD slice.

Lane `0.5` — methodology boundary, typed task lifecycle, tool
surface, command templates, multi-agent installer, self-hosting.
Encompasses:

- orchestration-flow and agent-tool-surface specs (authoritative)
- typed task + finding + evidence domain in `tanren-domain`
- typed tool schemas in `tanren-contract`
- event-store extensions + projections in `tanren-store`
- service layer + install renderer + format drivers + MCP server in
  `tanren-app-services` and `tanren-mcp`
- CLI subcommands (`install`, `task`, `finding`, `phase`, `issue`,
  `ingest`, `replay`) in `tanren-cli`
- rewritten command sources under `commands/spec/` and
  `commands/project/`
- self-hosted `.claude/commands/`, `.codex/skills/`,
  `.opencode/commands/` in the tanren repo

Lane 0.4 and Lane 0.5 scopes are disjoint: 0.4 ships no methodology
work; 0.5 ships no harness/environment/runtime implementation (those
are Phase 1+).
