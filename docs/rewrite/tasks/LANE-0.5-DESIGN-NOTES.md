# Lane 0.5 — Design Notes and Decision Log

> **Purpose.** This document is the durable, in-repo record of the design
> conversation that produced Lane 0.5's expanded scope. It captures the
> decisions made, the rationale behind each, the alternatives considered
> and rejected, and the implications for future phases. Read this before
> LANE-0.5-METHODOLOGY.md — the spec assumes the decisions recorded here.
>
> If this document drifts from the other canon docs, **this document is
> the authoritative source of intent**; fix the other docs.

---

## 1. Scope trajectory

Lane 0.5 started as a **docs-only methodology-boundary pass**. Over the
course of the design conversation it expanded to encompass the full
Phase-0 completion deliverable:

| Era | Scope |
|---|---|
| Original brief | Canon doc alignment + shared command markdown refactor |
| First expansion | + install-time templates with `{{DOUBLE_BRACE_UPPER}}` substitution |
| Second expansion | + `tanren install` Rust subcommand + self-hosting |
| Third expansion | + orchestration-flow spec with investigate-first failure routing |
| Fourth expansion | + typed task domain + tool surface + `.agent-status` retirement |
| Fifth expansion | + multi-guard task lifecycle (Gate/Audit/Adherence/…) + extensible pillars + typed evidence + multi-agent install parity (Claude Code, Codex Skills, OpenCode) + MCP transport |

The user directed each expansion explicitly. This document and the
accompanying architecture specs carry the full combined scope.

The non-negotiable "docs only" clause from earlier drafts **no longer
applies**. Lane 0.5 ships Rust code in several crates plus new binaries,
plus the templated command sources, plus the rendered install artifacts.

---

## 2. Core design decisions

Each decision has rationale, alternative considered, and why the
alternative was rejected.

### 2.1 Python orchestrator is reference-only

**Decision.** Python code under `packages/tanren-core/`, `services/`,
`tests/` is frozen reference material. It may rot; it may fail CI. It
will be deleted when the Rust path is functional. No compatibility is
maintained.

**Rationale.** Tanren 2.0 is a clean-room rewrite, not a port. Keeping
Python working in parallel doubled maintenance without adding value.

**Alternatives rejected.** (a) Full Python parity — rejected as
scope-creep with zero strategic payoff. (b) Partial parity (shared
config) — rejected as a silent coupling risk.

**Implication.** Any Rust design choice that would have required Python
compatibility hacks is free to pick the cleaner design.

### 2.2 Three interactive phases, everything else autonomous

**Decision.** Only `shape-spec`, `walk-spec`, and `resolve-blockers`
pause for a human. Every other spec-loop phase is autonomous.
`investigate` is the escalation mechanism; it loops autonomously until
its loop cap, then promotes to a blocker (the only path that triggers
`resolve-blockers`).

**Rationale.** Human intervention should be rare and purposeful.
Autonomy is the value proposition of the methodology; anything that
breaks it without payoff should go.

**Alternatives rejected.** (a) Human approval at every audit — rejected
as time-wasting. (b) No escalation ladder, just halt on any failure —
rejected as producing too-frequent halts.

**Implication.** `investigate` must be strong enough to resolve most
failures autonomously. `resolve-blockers` sees a narrow, high-signal
subset of problems.

### 2.3 Gate failure routes through investigate, not back to do-task

**Decision.** On persistent gate failure, the orchestrator dispatches
`investigate`, which decides among: `revise_task` (acceptance criteria
were wrong), `create_task` (new fix scope needed), or
`escalate_to_blocker`. Only then is `do-task` re-dispatched on a fresh
session.

**Rationale.** `do-task` has one job: implement a clean task
description. If it also has to debug a partial prior implementation,
it's doing two jobs badly. Splitting out investigation keeps each
phase's role sharp.

**Alternatives rejected.** (a) Loop back to `do-task` directly on gate
failure — rejected as conflating roles. (b) Inline debugging in
`audit-task` — rejected; audit's job is quality judgment, not diagnosis.

**Implication.** Every persistent failure costs one extra phase
(`investigate`), but payoff is a cleaner, more diagnosable agent
specialization.

### 2.4 Audit-task emits findings, not plan.md edits

**Decision.** `audit-task` produces typed findings via `add_finding`;
the orchestrator materializes new tasks (or reopens nothing) from
findings classified `fix_now`. Audit does NOT edit `plan.md`.

**Rationale.** Markdown mutation by agents has historically been
fragile (checkbox drift, whitespace noise, accidental full-file
rewrites). Typed findings + orchestrator-owned materialization
eliminates that failure mode.

**Alternatives rejected.** (a) Insert a second investigate phase between
audit and do-task — rejected; audit findings are already typed
investigation results.

**Implication.** `plan.md` becomes a generated view, not a live edit
surface.

### 2.5 Task state is monotonic; Complete is terminal

**Decision.** Task states: `Pending → InProgress → Implemented →
{GateChecked, Audited, Adherent, …} → Complete`, plus an `Abandoned`
side branch. `Complete` is terminal. Every spec-level failure
materializes *new* tasks with typed `origin`; no un-checking.

**Rationale.** Reopening a completed task discards history and makes
progress tracking dishonest. Provenance tracking in `TaskOrigin`
surfaces that "this new task exists because run-demo found a gap" far
more cleanly than a resurrected checkbox.

**Alternatives rejected.** (a) Toggle completed tasks back to
in-progress on later failure — rejected as erasing honest audit trail.

**Implication.** `plan.md` may grow over a spec's lifecycle; the task
list is append-only except through `Abandoned`.

### 2.6 Multi-guard task lifecycle

**Decision.** The forward transition from `Implemented` to `Complete`
is gated by a configurable set of independent guards (default:
`GateChecked`, `Audited`, `Adherent`). Each guard is satisfied by a
separate phase; guards can execute in any order and potentially in
parallel.

**Rationale.** Different quality dimensions are independent concerns.
A standards-adherence check doesn't depend on a rubric audit, and
either can legitimately run in parallel with the automated gate.
Modeling them as parallel guards removes serial bottlenecks and makes
adding new checks (`SecurityReviewed`, `LoadTested`, etc.) a
zero-refactor change.

**Alternatives rejected.** (a) Single linear chain
`Implemented → Audited → Adherent → Complete` — rejected; serializes
independent work and forces ordering that has no semantic meaning.
(b) Collapse all checks into one `Audit` phase — rejected; conflates
rubric-style opinionated judgment with pass/fail standards checking.

**Implication.** The state machine tracks per-guard satisfaction on
`Implemented+` state; `Complete` fires when all required guards are
satisfied. Required-guard set is config-defined in `tanren.yml` under
`methodology.task_complete_requires`.

### 2.7 Audit vs Adherence — distinct phases

**Decision.** `audit-task` / `audit-spec` apply opinionated pillar
rubrics (1–10, pass ≥ 7, target 10) with mandatory linked findings.
`adhere-task` / `adhere-spec` apply dynamic repo-authored standards
filtered to the current scope; pass/fail per standard; no scores.

**Rationale.** The two concerns have different semantics.
- Audit = "is this good work?" — judgment call, scored, rubric-driven.
- Adherence = "does this follow the rules the team set?" —
  deterministic, pass/fail, standard-driven.
Collapsing them produces either muddy rubric scores for compliance
checks or shallow pass/fail for judgment questions.

**Alternatives rejected.** (a) Use rubric scoring for standards —
rejected; standards are binary. (b) Use pass/fail for pillars —
rejected; pillars have a useful scored middle ground (7 passes, 10
targets).

**Implication.** `adhere-task` keeps standards compliance continuous;
the existing `triage-audits` batch command becomes a periodic backlog
curator rather than the primary standards enforcement mechanism.

### 2.8 Extensible pillars; findings required for gaps

**Decision.** Pillars live in a config file (`tanren/rubric.yml` or
fallback under `tanren.yml` `methodology.rubric:`). Built-in defaults
(13): completeness, performance, scalability, strictness, security,
stability, maintainability, extensibility, elegance, style, relevance,
modularity, documentation_complete. Users add/remove/override.

**Invariant.** `record_rubric_score(pillar, score)` with `score < 10`
requires at least one linked finding. If `score < passing` (default
7), at least one linked finding must have severity `fix_now`. Enforced
at tool-call time; the call fails with a typed error if violated.

**Rationale.** Prevents "security: 2" with empty findings — scores
without supporting evidence are not useful. Forces the auditor to
either improve the score or justify the deficit with concrete
problems. The 10-point scale with 7 as passing shoots high (10
target) without gating on nits.

**Alternatives rejected.** (a) 1–5 scale — rejected; too coarse,
loses the "acceptable but improvable" middle. (b) Fixed 13-pillar set
— rejected; teams need extensibility (e.g., "observability" pillar
for platform code). (c) Scores without linked findings — rejected;
allows empty narrative scoring.

**Implication.** Audits are more rigorous but also more actionable;
every gap is a concrete improvement opportunity.

### 2.9 Tool-surface-first artifact model

**Decision.** All structured agent-produced state (tasks, findings,
rubric scores, evidence frontmatter, signposts, demo steps) passes
through typed tools. Agents never write structured data directly to
disk. Free markdown body text remains agent-authored.

**Transports.** Two, sharing one service:
1. **MCP (primary).** `tanren-mcp` Rust binary using the `rmcp`
   SDK (`modelcontextprotocol/rust-sdk`), stdio transport, tool
   registration via `#[tool_router]` + `#[tool]` attribute macros.
2. **CLI fallback.** `tanren-cli` subcommands (`tanren task create …`,
   `tanren finding add …`, `tanren phase outcome …`) for Bash-tool
   invocation when MCP isn't wired. Same service, same events, same
   schema.

Every tool call appends a typed event to
`{spec_folder}/phase-events.jsonl` (a committed permanent artifact) and
applies the event to the store.

**Rationale.** Agents writing raw markdown with structured
expectations is a historical failure mode. Tools enforce schema at
construction time; invalid state cannot be produced. Dual transport
means self-hosting works even before MCP is fully wired.

**Alternatives rejected.** (a) Agents write JSON files, orchestrator
parses — rejected; validation happens too late (at read, not at
write). (b) MCP-only — rejected; constrains early self-hosting to
agents with MCP configured. (c) Hand-rolled stdio server — rejected;
`rmcp` is the official Rust SDK and 1500–3000 LoC of protocol
maintenance is not worth saving.

**Implication.** Retires `.agent-status` files, markdown checkbox
parsing, and any "agent edits plan.md" pattern. Every interaction
between agent and orchestrator is typed.

### 2.10 Schema enforcement at the tool boundary

**Decision.** Every tool validates its input serde-schema before
dispatch. Invalid input returns a typed `ToolError { field_path,
expected, actual, remediation }` to the agent. The agent gets
actionable feedback; no invalid structured state ever reaches disk or
the store.

**Rationale.** Postflight validation surfaces errors too late to
give the agent a chance to recover within the same session. Tool-
boundary validation gives immediate feedback with enough context to
fix.

**Alternatives rejected.** (a) Postflight-only validation — rejected;
slower recovery, worse error locality. (b) No validation — rejected;
defeats the purpose of typed tools.

**Implication.** Tool-surface definitions in `tanren-contract` are
the single source of truth for schemas; all transports share them.

### 2.11 Per-phase tool capability scopes

**Decision.** Each phase is granted a typed capability set at
dispatch time. The MCP server and CLI consult
`TANREN_PHASE_CAPABILITIES` (or a typed equivalent) and reject
out-of-scope calls with `CapabilityDenied`. Example:
- `do-task` → `{start_task, complete_task, add_signpost, …,
  report_phase_outcome}`
- `audit-task` → `{add_finding, record_rubric_score,
  record_non_negotiable_compliance, list_tasks, report_phase_outcome}`
- `investigate` → `{revise_task, create_task, add_finding,
  escalate_to_blocker, list_tasks, report_phase_outcome}`
- `triage-audits` → `{create_issue, add_finding,
  report_phase_outcome}` (explicitly NOT `create_task`)

**Rationale.** Prevents a misbehaving `audit-task` from, say, creating
tasks it shouldn't, or a `handle-feedback` from escalating directly to
the human without going through investigate first. Capability scoping
is the security model for the orchestration layer.

**Alternatives rejected.** (a) One uniform tool set for all phases —
rejected; over-permissive. (b) Runtime tool-call checks in each
service method — rejected; capability is a cross-cutting concern best
enforced at the transport layer.

**Implication.** `escalate_to_blocker` is callable only from
`investigate`. `create_task` is callable only from `shape-spec`,
`investigate`, and `resolve-blockers`. `create_issue` is callable only
from `triage-audits` and `handle-feedback(out-of-scope)`.

### 2.12 `plan.md` (and friends) are orchestrator-owned; three-layer enforcement

**Decision.** Orchestrator-owned files (`plan.md`, `progress.json`,
generated indexes) cannot be edited by agents. Enforcement:
1. Prompt banner rendered into every agent prompt: "⚠️ {FILE} is
   orchestrator-owned; any edits will be reverted and recorded as an
   UnauthorizedArtifactEdit event."
2. Filesystem `chmod 0444` on agent-session start; restored on exit.
3. Postflight diff + auto-revert; emits
   `UnauthorizedArtifactEdit { file, diff_preview, phase,
   agent_session }` to the event log.

**Rationale.** Single-layer enforcement has historically failed —
chmod can be bypassed with `chmod +w`; prompt banners can be ignored;
postflight alone lets partial writes damage things before detection.
Three layers provide defence in depth.

**Alternatives rejected.** (a) Chmod only — rejected; bypassable.
(b) Prompt only — rejected; unreliable. (c) Postflight only —
rejected; accepts bad writes silently before catching them.

**Implication.** A `UnauthorizedArtifactEdit` event is a strong signal
in the event log — a repeated pattern suggests an agent needs its
prompt or capabilities adjusted.

### 2.13 Typed evidence documents

**Decision.** Every evidence file uses YAML frontmatter + markdown
body. Frontmatter schema is typed in Rust, managed exclusively via
tools. Narrative body remains free agent prose.

Files:
- `spec.md` — `SpecFrontmatter` via `set_spec_*`, `add_spec_*` tools
- `plan.md` — `PlanFrontmatter`, generated by orchestrator only
- `demo.md` — `DemoFrontmatter` via `add_demo_step`, `mark_demo_step_skip`, `append_demo_result`
- `audit.md` — `AuditFrontmatter` via `record_rubric_score`, `record_non_negotiable_compliance`
- `signposts.md` — `SignpostsFrontmatter` via `add_signpost`, `update_signpost_status`
- `investigation-report.json` — `InvestigationReport`, generated from tool calls
- `phase-events.jsonl` — append-only event log; every line is a typed tool call

**Rationale.** Typed frontmatter enables downstream consumers (a
future web UI, audit review API, spec-review dashboard) without
re-architecture. Body text stays free for narrative nuance.

**Alternatives rejected.** (a) Free-form markdown throughout —
rejected; blocks structured downstream tooling. (b) Full structured
files (no narrative) — rejected; loses human readability.

**Implication.** Every agent-facing evidence authoring operation
becomes a tool call; the markdown body is the one place agents write
freely.

### 2.14 Multi-agent install parity, with research-backed format drivers

**Decision.** `tanren install` renders a single shared command source
into three agent-specific formats simultaneously:
- Claude Code: `.claude/commands/<name>.md` — YAML frontmatter + markdown body, MCP config at `.mcp.json` (JSON).
- Codex Skills: `.codex/skills/<name>/SKILL.md` — **directory per
  command**, YAML frontmatter + markdown body, MCP config at
  `.codex/config.toml` (**TOML**, `[mcp_servers.<name>]`).
- OpenCode: `.opencode/commands/<name>.md` — YAML frontmatter with
  **prompt body inside the `template` field** (not the markdown body),
  MCP config at `opencode.json` (JSON).

The renderer has a pluggable `InstallTargetFormat` trait with shipped
drivers for each target, plus three config-only drivers
(`ClaudeMcpJson`, `CodexConfigToml`, `OpenCodeJson`).

**Rationale.** Three format divergences — format dispatch (YAML/JSON/
TOML), file-vs-directory, prompt-body location — demand format
drivers, not simple path multiplexing. Researching the actual 2026
conventions of each tool prevented shipping broken assumptions.

**Alternatives rejected.** (a) Ship Claude Code only, add others
later — rejected; user explicitly requested day-one parity.
(b) Assume all three use the same format — rejected; research proved
otherwise (see §5.2 below).

**Implication.** Single shared source, many rendered outputs, perfect
semantic parity. User edits to installed files are destructive-on-
reinstall by design; customization = fork `commands/`.

### 2.15 Per-target merge policy: commands destructive, standards preserving

**Decision.** Install targets declare a merge policy:
- `destructive` — overwrite on reinstall. Used for commands (tanren
  is opinionated about workflow).
- `preserve_existing` — only create missing files; never overwrite.
  Used for standards baselines (repo tailors standards to its own
  needs).
- `preserve_other_keys` — for config files (`.mcp.json`, etc.); only
  overwrite the tanren-owned section, leave user's other keys alone.

**Rationale.** Commands are the workflow engine — Tanren is
opinionated. Standards are repo culture — tanren should not stomp
them. Config files often have other tools' entries — tanren should
not destroy them.

**Alternatives rejected.** (a) All destructive — rejected; clobbers
standards and MCP configs. (b) All preserving — rejected; commands
would drift silently.

**Implication.** Repo-specific customization workflow: fork
`commands/`; modify standards in place; keep MCP configs clean.

### 2.16 Cross-spec concerns (data model now, resolution later)

**Decision.** Lane 0.5 encodes the typed events for cross-spec
concerns (merge conflicts, intent conflicts, stacked-diff dependent
specs) but does not implement resolution. `SpecDefined` event carries
`base_branch`, `depends_on_spec_ids`, and `touched_symbols`;
`CrossSpecIntentConflict` and `CrossSpecMergeConflict` event variants
exist. Resolution engine is Phase 2+ work.

**Rationale.** Teams run multiple specs in parallel; merged Spec B
can invalidate Spec A's completeness without text conflict (the
"add foo to every bar / add 3 new bars" example). Encoding the events
now ensures Phase 2+ can add resolution without a data migration.

**Alternatives rejected.** (a) Ignore cross-spec concerns until
Phase 2+ — rejected; data model retrofitting is expensive.
(b) Implement resolution now — rejected; scope explosion.

**Implication.** Future lanes build the rebase/reconciliation engine
atop the already-present events.

### 2.17 Retries are fresh sessions

**Decision.** Every retry dispatches a new agent session. No resume.

**Rationale.** Agent drift on resume is the dominant observable
failure mode. Prompt caching keeps fresh-session cost low.

**Alternatives rejected.** (a) Resume with conversation history —
rejected; drift.

**Implication.** Investigate's revised task description (or the
previous failure's narrative) becomes the retry session's context.

### 2.18 triage-audits creates issues, not tasks

**Decision.** `triage-audits` emits `create_issue(...)` for each
user-approved finding group, not `create_task(...)`. Issues go to
the external tracker as backlog; they can later be shaped into specs
via `shape-spec`.

**Rationale.** Triaged standards audits surface repo-wide concerns
too broad for the current spec's scope. Creating backlog *issues*
(future specs) honors the spec boundary; creating tasks would
contaminate whatever spec happened to be active.

**Alternatives rejected.** (a) Create tasks scoped to the current
spec — rejected; violates spec boundary.
(b) Create neither (just log findings) — rejected; loses
actionability.

**Implication.** Triage-audits is a backlog curator. Its outputs
feed the next `shape-spec` session, not the current execution.

### 2.19 Self-hosting is a tanren-repo convention, not prescriptive

**Decision.** The `just install-commands` + `install-commands-check`
+ `just ci` drift-gate recipes are tanren-repo-specific dogfooding.
They ensure tanren's own installed commands don't drift from
`commands/`. Downstream adopters (forgeclaw, etc.) run `tanren
install` however they like — CI integration, drift gates, and
justfile recipes are their choice.

**Rationale.** Prescribing downstream CI conventions makes tanren
intrusive. The install CLI is the product; dogfooding is how we
validate it.

**Alternatives rejected.** (a) Ship `tanren init` that writes
recipes to the user's justfile — rejected; imposes our tool choice
(just) and our CI shape on them.

**Implication.** `tanren install` behavior is pure; our justfile is
the validation harness.

### 2.20 Four pillars of rubric pass

**Decision.** A rubric pass requires all four of:
1. Every applicable pillar scores ≥ 7.
2. Every non-negotiable compliance check = `pass`.
3. Demo signal = `pass`.
4. Zero unaddressed `fix_now` findings.

Tasks with non-target pillar scores (7–9) that defer findings to
backlog (via `create_issue`) still pass; deferred findings feed the
next triage-audits or shape-spec cycle.

**Rationale.** Prevents "all 7s but missing non-negotiables" passes,
prevents "all pillars pass but demo broken" passes. The four-way AND
is the honest minimum.

**Alternatives rejected.** (a) Pillars-only — rejected; ignores
non-negotiables and demo. (b) Binary pass (10/10 everywhere) —
rejected; blocks on nits.

**Implication.** `audit-spec` must explicitly record all four
outcomes via tools; missing any is a typed error.

---

## 3. Orchestration flow summary

Condensed reference; full detail in
`docs/architecture/orchestration-flow.md`.

**Single-spec happy path:**
```
shape-spec INTERACTIVE
  → SETUP
  → (loop) DO_TASK → TASK_GATE + AUDIT_TASK + ADHERE_TASK (parallel guards)
                   → TaskCompleted
  → (no pending tasks)
  → SPEC_GATE + RUN_DEMO + AUDIT_SPEC + ADHERE_SPEC (parallel)
  → walk-spec INTERACTIVE
  → orchestrator creates PR
  → CI + review
  → handle-feedback as needed
  → orchestrator merges PR when clean
  → CLEANUP
```

**Failure routing:**
```
TASK_GATE fail → INVESTIGATE_TASK → {revise, create_task, escalate}
AUDIT_* findings → orchestrator create_task(origin=Audit) → loop
DEMO fail → INVESTIGATE_SPEC → create_task(origin=SpecInvestigation)
ADHERE_* findings → create_task(origin=Adherence) → loop
investigate loop cap → escalate_to_blocker → resolve-blockers
```

**Cross-spec:**
```
parallel_spec_merged → postflight reconciliation
  → text conflict → INVESTIGATE_MERGE_CONFLICT
  → intent conflict → INVESTIGATE_INTENT_CONFLICT → create_task(origin=CrossSpecIntent)
```

---

## 4. Tool surface summary

Full catalog in `docs/architecture/agent-tool-surface.md`. Summary by
capability group:

| Group | Tools |
|---|---|
| Task ops | `create_task`, `start_task`, `complete_task`, `revise_task`, `abandon_task`, `list_tasks` |
| Findings + rubric | `add_finding`, `record_rubric_score`, `record_non_negotiable_compliance` |
| Spec frontmatter | `set_spec_title`, `set_spec_non_negotiables`, `add_spec_acceptance_criterion`, `set_spec_demo_environment`, `set_spec_dependencies`, `set_spec_base_branch` |
| Demo frontmatter | `add_demo_step`, `mark_demo_step_skip`, `append_demo_result` |
| Signposts | `add_signpost`, `update_signpost_status` |
| Phase lifecycle | `report_phase_outcome`, `escalate_to_blocker` (investigate only), `post_reply_directive` (handle-feedback only) |
| Backlog | `create_issue` |
| Adherence | `list_relevant_standards`, `record_adherence_finding` |

---

## 5. Research that shaped the plan

### 5.1 Existing Python orchestration

Read via agent survey of `packages/tanren-core/`:
- Phase enum in `schemas.py:14–24`
- Signal-driven transitions in `worker.py:311–360`, `signals.py:93–142`
- Gate resolution priority chain in `env/gates.py`
- Retry model (`_MAX_RETRIES = 3`, backoff `(10, 30, 60)`) in
  `worker.py:82`
- 15 existing commands with identifiable workflow leaks (git/gh/make
  inlined, `.agent-status` writes, plan.md mutations, GraphQL calls,
  PR/comment creation)

The Python implementation is preserved as reference-only (§2.1).

### 5.2 Agent framework format research (2026-04-17)

- **Claude Code.** `.claude/commands/<name>.md`, YAML frontmatter +
  markdown body, MCP config at `.mcp.json`.
- **Codex CLI.** Modern: Skills — `.codex/skills/<name>/SKILL.md`
  (directory per command). MCP
  config in **TOML** at `.codex/config.toml`
  (`[mcp_servers.<name>]`). `AGENTS.md` is a separate convention for
  shared instructions, not commands.
- **OpenCode.** `.opencode/commands/<name>.md`, YAML frontmatter with
  the **prompt body inside the `template` field**, MCP config at
  `opencode.json`.

**Implication.** Single flat multiplexed install is impossible.
Format drivers per target are required.

### 5.3 Rust MCP SDK

- **`rmcp`** — `modelcontextprotocol/rust-sdk`, de-facto official,
  v1.3.0+ in early 2026, tokio-based, attribute-macro tool
  registration, stdio/SSE/HTTP transports. License to be verified
  against `deny.toml` at implementation time.
- Alternatives (`rust-mcp-sdk`, `mcp-protocol-sdk`, `mcp-sdk-rs`) are
  smaller / less maintained.
- Hand-rolling: 1500–3000 LoC estimated; not worth the spec-drift
  maintenance cost.
- Pitfall: **never write to stdout in stdio-transport MCP**
  (corrupts JSON-RPC framing). Workspace lints forbid
  `println!`/`eprintln!`/`dbg!`; use `tracing_subscriber::fmt()
  .with_writer(std::io::stderr)`.

---

## 6. Phase 1+ implications

Lane 0.5 leaves the following explicit handoffs to Phase 1+:

1. **Runtime wiring for MCP.** Lane 0.5 ships the `tanren-mcp` binary
   and a functional tool surface; Phase 1 lands the harness /
   environment lease infrastructure that actually invokes agents
   against it.
2. **Daemon parity.** Install is CLI-local for Phase 0 exit. Exposing
   install through the daemon + API is deferred; the service API
   surface in `tanren-app-services::methodology` is already shaped so
   this is a transport addition, not an architectural change.
3. **Cross-spec resolution engine.** Typed events for merge and intent
   conflicts are in Lane 0.5; the actual rebase/reconciliation engine
   is Phase 2+.
4. **Task-store-driven plan.md rendering.** Lane 0.5 declares plan.md
   orchestrator-owned and generates it from events; polish and
   per-spec visualizations (e.g., dep-graph rendering, milestone
   rollups) are later lanes.
5. **Issue-provider adapters.** Lane 0.5 defines the tool catalog
   (`create_issue`, `post_reply_directive`) and relies on adapters
   being implemented for the first time in Phase 1+. GitHub is the
   initial target; Linear is planned (see user_role memory).
6. **Python removal.** Scheduled once the Rust path demonstrates
   parity across a real self-hosted spec cycle. Phase 1 decision
   point.

---

## 7. Non-goals for Lane 0.5

To prevent scope drift:

- No harness / environment-lease implementation.
- No planner-native orchestration (still single-spec sequential).
- No Phase 1+ runtime.
- No final enterprise governance model (policy rules, budgets,
  placement) — comes in Phase 3.
- No Linear issue adapter (GitHub only for this lane; Linear is a
  follow-up lane per user_role memory).
- No Python compatibility work.
- No downstream-consumer CI recipe prescription.
- No non-stdio MCP transports.
- No on-disk secret handling (MCP is local, secrets flow via existing
  `secrecy::Secret<T>` surfaces).

---

## 8. Audit dimensions specific to Lane 0.5

Beyond the global 10-pillar rubric, Lane 0.5 audits must check:

1. **Boundary clarity.** Tanren-code vs Tanren-markdown split is
   unambiguous across every doc.
2. **Typed state monotonicity.** Complete is terminal in property
   tests; no code path produces an illegal transition.
3. **Tool-boundary validation.** Invalid inputs to every tool return
   typed `ToolError`s with informative `remediation`.
4. **Capability enforcement.** Out-of-scope tool calls rejected with
   `CapabilityDenied`.
5. **Guard independence.** Out-of-order guard events converge to the
   correct `Implemented+` state.
6. **Multi-target parity.** Rendered content across
   `.claude/commands`, `.codex/skills`, `.opencode/commands` is
   semantically identical (canonicalized hash equal).
7. **Standards preserve policy.** Hand-edits to standards survive
   reinstall.
8. **Self-hosting drift.** `just install-commands-check` on the
   tanren repo passes.

---

## 9. Pointers

- Full Lane 0.5 execution plan (living): `~/.claude/plans/read-the-
  instructions-at-sunny-starlight.md` (agent-scoped; this doc is its
  permanent in-repo reflection).
- Spec: [LANE-0.5-METHODOLOGY.md](LANE-0.5-METHODOLOGY.md)
- Brief: [LANE-0.5-BRIEF.md](LANE-0.5-BRIEF.md)
- Audit dimensions: [LANE-0.5-AUDIT.md](LANE-0.5-AUDIT.md)
- Orchestration flow: [../../architecture/orchestration-flow.md](../../architecture/orchestration-flow.md)
- Tool surface: [../../architecture/agent-tool-surface.md](../../architecture/agent-tool-surface.md)
- Evidence schemas: [../../architecture/evidence-schemas.md](../../architecture/evidence-schemas.md)
- Audit rubric: [../../architecture/audit-rubric.md](../../architecture/audit-rubric.md)
- Adherence: [../../architecture/adherence.md](../../architecture/adherence.md)
- Install targets: [../../architecture/install-targets.md](../../architecture/install-targets.md)
- Methodology boundary: [../METHODOLOGY_BOUNDARY.md](../METHODOLOGY_BOUNDARY.md)
- Phase taxonomy: [../../architecture/phase-taxonomy.md](../../architecture/phase-taxonomy.md)
