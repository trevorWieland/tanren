# Methodology System

Tanren's methodology layer gives agents project memory and execution discipline.

## Command Files

The `commands/` directory defines 15 workflow commands with phase-specific
instructions, output expectations, and failure handling.

| Command | Role | Primary Purpose |
|---|---|---|
| `shape-spec` | conversation | Decompose issue into spec + executable plan |
| `do-task` | implementation | Implement the supplied task scope and emit evidence |
| `audit-task` | audit | Audit the supplied task scope and emit findings |
| `run-demo` | implementation | Execute the supplied demo context and record results |
| `audit-spec` | audit | Perform whole-spec review and classify findings |
| `walk-spec` | conversation | Interactive developer walkthrough |
| `handle-feedback` | feedback | Process PR comments and iterate |
| `resolve-blockers` | conversation | Diagnose blockers |
| `investigate` | conversation | Deep technical investigation |
| `plan-product` | conversation | Create product docs for new project |
| `discover-standards` | audit | Propose standards from repo patterns |
| `inject-standards` | implementation | Apply standards updates |
| `index-standards` | implementation | Rebuild standards index |
| `triage-audits` | audit | Prioritize audit backlog |
| `sync-roadmap` | conversation | Align roadmap with real state |

## Agent Roles

Tanren's workflow uses role-specialized agents with clear execution boundaries:

- `conversation`: shape specs, clarify requirements, and coordinate with developers
- `implementation`: execute planned tasks and produce code/documentation changes
- `audit`: validate outputs against spec intent and quality standards
- `feedback`: triage and apply PR review feedback
- `conflict-resolution`: resolve merge conflicts using spec intent and dependency context

## Standards Profiles

Profiles in `profiles/` package standards by stack.

- `default`: language-agnostic baseline
- `python-uv`: strict typing, testing, architecture, naming, and dependency conventions

## Ownership Boundary

The methodology layer is split deliberately:

- **Tanren code** owns workflow mechanics, provider integration, workflow
  target selection, verification-hook resolution, and repo-specific installed
  command rendering.
- **Command markdown** owns agent instructions, allowed edits, required
  outputs, and role behavior.

Shared command markdown should describe:

- what context the agent must consume
- what files it may change
- what artifact(s) it must produce

Shared command markdown should not hardcode:

- issue tracker shell commands
- branch creation steps
- commit / push / PR steps
- literal verification commands
- “discover the next task” workflow logic

## Product Templates

`templates/product/` provides `mission.md`, `roadmap.md`, and tech-stack/
conventions templates used to seed persistent product context.

## Role Separation

Role independence is deliberate: implementation and audit should use different
model families when possible to reduce self-agreement bias.

## Agent Agnosticism

Commands describe **capabilities needed**, not tools or models by name.

- `**Suggested model:**` lines describe the reasoning profile (strong planner,
  fast implementer, independent auditor) and execution mode (interactive vs
  autonomous) — never a specific model name or provider.
- User interaction is described as behavior ("ask the user", "present options",
  "wait for response") — never as a specific tool invocation.
- CLI references use generic terms ("agent CLI", "installed CLIs") — never
  specific product names.

This ensures commands work identically across Claude Code, Codex CLI, OpenCode,
Aider, and any future agent runtime.
