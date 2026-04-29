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

Tanren's method is layered:

```text
plan-product -> identify-behaviors -> craft-roadmap -> shape-spec -> orchestrate -> walk-spec
```

Project-planning commands maintain product intent, behavior canon, and roadmap
state. Spec-loop commands execute one roadmap DAG node at a time. The
orchestration state machine is therefore the execution layer of the method,
not the source of product direction.

Shared command sources live under `commands/`. The active command surface is
currently the spec loop:

- **`commands/spec/`** — commands that participate in the spec-
  orchestration state machine. Each emits typed events via the tool
  surface and contributes to task / finding state.
- **`commands/project/`** — temporary project-method bootstrap commands that
  operate outside the spec loop. They render via `tanren-cli install`, but are
  not sequenced by the orchestrator's task/spec state machine.

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

### Project-management commands

Current project commands are temporary bootstrap commands. They directly edit
planning artifacts and must not be treated as native typed Tanren phases.

| Command | Role | Autonomy | Notes |
|---|---|---|---|
| `plan-product` | product planning | interactive | maintains product brief, personas, concepts, and README framing |
| `identify-behaviors` | behavior planning | interactive | creates behavior files and updates behavior status |
| `craft-roadmap` | roadmap synthesis | interactive | creates or revises temporary roadmap DAG and human roadmap |

Previously scaffolded commands that lack proof of function have been removed.
Fresh product-method and project-analysis commands should be added
deliberately. The current product-method commands are intentionally temporary
and should be replaced once typed artifacts, validators, tools, and
project-method events exist.

Future project-analysis commands should cover non-interactive scheduled or
manual sweeps such as standards audits, security analysis, mutation-testing
classification, and post-ship health review. Their outputs should be typed
findings or proposed planning changes, not direct mutation of active spec task
lists.

## Command Inventory

Current source commands are intentionally narrow:

| Command | Source | Enacted status |
|---|---|---|
| `shape-spec` | `commands/spec/shape-spec.md` | active spec-loop command |
| `do-task` | `commands/spec/do-task.md` | active spec-loop command |
| `audit-task` | `commands/spec/audit-task.md` | active spec-loop command |
| `adhere-task` | `commands/spec/adhere-task.md` | active spec-loop command |
| `run-demo` | `commands/spec/run-demo.md` | active spec-loop command |
| `audit-spec` | `commands/spec/audit-spec.md` | active spec-loop command |
| `adhere-spec` | `commands/spec/adhere-spec.md` | active spec-loop command |
| `walk-spec` | `commands/spec/walk-spec.md` | active spec-loop command |
| `handle-feedback` | `commands/spec/handle-feedback.md` | active spec-loop command |
| `investigate` | `commands/spec/investigate.md` | active spec-loop command |
| `resolve-blockers` | `commands/spec/resolve-blockers.md` | active spec-loop command |
| `plan-product` | `commands/project/plan-product.md` | temporary project-method command |
| `identify-behaviors` | `commands/project/identify-behaviors.md` | temporary project-method command |
| `craft-roadmap` | `commands/project/craft-roadmap.md` | temporary project-method command |

Automated phases such as `setup`, `task-gate`, `spec-gate`, and `cleanup` are
runtime phases, not shared command markdown.

Native project-method commands are not implemented yet:

| Command family | Status |
|---|---|
| `plan-product` | temporary markdown command; native typed command planned |
| `identify-behaviors` | temporary markdown command; native typed command planned |
| `craft-roadmap` | temporary markdown command; native typed command planned |
| project-analysis sweeps | planned future command family for scheduled/static audits |

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

## Product Method Context

Product context is not one-shot scaffolding. Tanren must support both fresh
projects and existing codebases by maintaining a product brief, accepted
behavior catalog, and roadmap DAG over time. New client requests, bug reports,
post-ship outcomes, and business changes should route through this product
method layer before becoming shaped specs.

The same rule applies to proactive analysis. A scheduled standards sweep,
security audit, mutation-testing run, or health check may discover important
work, but it should enter Tanren as typed evidence and proposed behavior,
roadmap, or spec changes. Automated analysis can reduce discovery latency; it
does not replace behavior canon, roadmap DAG ordering, or human approval where
the project policy requires it.
