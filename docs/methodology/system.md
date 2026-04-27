# Methodology System

Tanren's methodology layer gives agents project memory and execution
discipline. Methodology is a strictly typed Rust control-plane concern;
shared command markdown is a templated agent-behavior layer rendered per
repo and per agent framework.

Canon pointers:
[architecture/orchestration-flow.md](../architecture/orchestration-flow.md),
[architecture/agent-tool-surface.md](../architecture/agent-tool-surface.md),
[architecture/evidence-schemas.md](../architecture/evidence-schemas.md),
[architecture/audit-rubric.md](../architecture/audit-rubric.md),
[architecture/adherence.md](../architecture/adherence.md),
[architecture/install-targets.md](../architecture/install-targets.md),
[architecture/phase-taxonomy.md](../architecture/phase-taxonomy.md).

## Command Organization

Shared command sources live under `commands/` in two subdirectories:

- **`commands/spec/`** — commands that participate in the spec-
  orchestration state machine. Each emits typed events via the tool
  surface and contributes to task / finding state.
- **`commands/project/`** — project-management commands that operate
  outside the spec loop. They still render via `tanren-cli install` but
  are not sequenced by the orchestrator's state machine. They may
  record typed project-governance state and backlog issues, but they
  do not mutate the active spec task list.

### Spec-loop commands (`commands/spec/`)

| Command | Role | Autonomy | Guard emitted on success |
|---|---|---|---|
| `shape-spec` | conversation | interactive | — |
| `do-task` | implementation | autonomous | TaskImplemented |
| `audit-task` | audit (rubric) | autonomous | TaskAudited |
| `adhere-task` | audit (adherence) | autonomous | TaskAdherent |
| `run-demo` | implementation | autonomous | — (spec-level) |
| `audit-spec` | audit (rubric) | autonomous | — (spec-level) |
| `adhere-spec` | audit (adherence) | autonomous | — (spec-level) |
| `walk-spec` | conversation | interactive | — |
| `handle-feedback` | feedback | autonomous | — |
| `investigate` | triage | autonomous | — (task-scope root cause; spec-scope tasks / escalation) |
| `resolve-blockers` | conversation | interactive | — |

### Project-management commands (`commands/project/`)

| Command | Role | Autonomy | Notes |
|---|---|---|---|
| `sync-roadmap` | reconciliation | autonomous (once consuming real spec state) | reads store + issue source; emits diff directives |
| `triage-audits` | audit curation | interactive | consumes batch standards report; emits `create_issue` (backlog), never `create_task` |
| `discover-standards` | standards authoring | interactive | authors new standards in `STANDARDS_ROOT` |
| `index-standards` | standards index | interactive | maintains standards index |
| `inject-standards` | standards context | interactive | injects relevant standards into context |
| `plan-product` | product authoring | interactive | authors product docs in `PRODUCT_ROOT` |

## Ownership Boundary

Every workflow concern is owned by **Tanren code**:

- workflow target resolution
- verification-hook resolution (command/phase-keyed with priority
  chain)
- issue-tracker operations, candidate selection, dependency mutation,
  roadmap/progress updates
- branch prep, commit/push/PR workflow, SCM mechanics
- task state transitions and finding routing (typed state machine,
  multi-guard completion)
- evidence frontmatter management (typed schemas via tools)
- orchestrator-owned artifact enforcement (three-layer read-only)
- template-variable rendering per install target
- MCP server registration and tool capability scoping

Every agent concern is owned by **shared command markdown**:

- role description
- input enumeration (from dispatch context)
- responsibility prose (opinionated, directive)
- output declaration (which tools to call; which narrative body
  files to author)
- out-of-scope list

## Agent ↔ Orchestrator Tool Surface

Every structured state mutation goes through a typed tool (MCP
primary via `tanren-mcp`, CLI fallback via `tanren-cli`). Full
catalog: [agent-tool-surface.md](../architecture/agent-tool-surface.md).

Core groups:
- Task ops (`create_task`, `start_task`, `complete_task`,
  `revise_task`, `abandon_task`, `list_tasks`)
- Findings + rubric (`add_finding`, `record_rubric_score`,
  `record_non_negotiable_compliance`)
- Spec / demo frontmatter (`set_spec_*`, `add_spec_*`,
  `add_demo_step`, `mark_demo_step_skip`, `append_demo_result`)
- Signposts (`add_signpost`, `update_signpost_status`)
- Phase lifecycle (`report_phase_outcome`, `escalate_to_blocker`
  (investigate-only), `post_reply_directive`
  (handle-feedback-only))
- Backlog (`create_issue`)
- Adherence (`list_relevant_standards`,
  `record_adherence_finding`)

Tools are phase-capability-scoped; out-of-scope calls return
`CapabilityDenied`. Schema validation happens at the tool boundary;
invalid inputs return typed `ToolError`s with `remediation`.

## Installed Artifacts

`tanren-cli install` renders `commands/` into per-agent-framework
destinations:
- `.claude/commands/<name>.md` (Claude Code)
- `.codex/skills/<name>/SKILL.md` (Codex Skills — directory per
  command)
- `.opencode/commands/<name>.md` (OpenCode — prompt body in
  `template` frontmatter field)

Plus MCP server registrations:
- `.mcp.json` (Claude Code)
- `.codex/config.toml` (Codex — TOML, `preserve_other_keys`)
- `opencode.json` (OpenCode)

Standards baselines install with `preserve_existing` policy (never
overwrite user customizations). Commands install destructively
(tanren is opinionated about workflow).

See [install-targets.md](../architecture/install-targets.md).

## Role Separation

Role independence remains deliberate: implementation and audit
should use different model families when possible to reduce
self-agreement bias. Adherence and Audit are distinct phases (see
[adherence.md](../architecture/adherence.md)) so their execution can
use different reasoning profiles.

## Agent Agnosticism

Commands describe **capabilities needed**, not tools or models by
name.

- `**Suggested model:**` lines describe the reasoning profile
  (strong planner, fast implementer, independent auditor) and
  execution mode (interactive vs autonomous) — never a specific
  model name or provider.
- User interaction is described as behavior ("ask the user",
  "present options", "wait for response") — never as a specific
  tool invocation.
- CLI references use `{{AGENT_CLI_NOUN}}` (default "the agent CLI")
  — never specific product names.

This ensures commands work identically across Claude Code,
Codex Skills, OpenCode, and any future agent runtime.

## Standards Profiles

Profiles in `profiles/` package standards by stack. Install-time
the appropriate profile is copied into `STANDARDS_ROOT` with
`preserve_existing` policy so repo-specific customization persists.

## Product Context

`plan-product` owns initial product-context authoring. It creates
`PRODUCT_ROOT` documents directly from user input and installed
standards rather than from separate repository templates.
