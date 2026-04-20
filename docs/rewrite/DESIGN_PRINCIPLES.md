# Tanren Clean-Room Rewrite: Design Principles

These principles are the decision filter for the rewrite. If two designs appear
equivalent, choose the one that aligns better with these principles.

## 1. Contract-First, Interface-Second

Define one canonical domain contract, then generate or strictly validate every
interface against it.

Implication:

- CLI/API/MCP/TUI parity is enforced by construction, not by convention.

## 2. Planner-Native Orchestration

Execution mechanics are necessary but insufficient. Planning, decomposition,
dependency handling, and replanning are first-class orchestration concerns.

Implication:

- "Dispatch a step" is a primitive, not the orchestration strategy.

## 3. Harness-Agnostic by Design

No harness-specific behavior may leak into core orchestration state transitions.

Implication:

- Claude Code, Codex, OpenCode, and future harnesses are adapters with
  capability declarations and normalized telemetry/error mapping.

## 4. Environment-Agnostic by Design

Execution substrate choice is a policy/scheduling decision, not hardcoded flow.

Implication:

- Local worktree, local Docker, DooD, and remote VM/cloud all implement one
  lease lifecycle contract.

## 5. Policy Is a First-Class Runtime

Authorization, budget, quota, and placement are enforced as typed decisions,
not scattered conditional checks.

Implication:

- Every denied action has an explicit decision record and reason.

## 6. Separation of Config and Secrets with Explicit Scopes

Treat configuration and secrets differently, and scope both explicitly across:

- project
- developer
- organization

Implication:

- No secret values in project-committed config.
- Source precedence is deterministic and auditable.

## 7. Deterministic Failure Semantics

Operational preconditions and lifecycle guard failures are domain-level outcomes,
not generic internal errors.

Implication:

- Conflict/precondition/policy failures are explicit and interface-consistent.

## 8. Event-Sourced Durability, Projection-Driven Read Performance

Canonical history is append-only events; operational performance comes from
purpose-built read models.

Implication:

- Avoid scan-heavy operational paths for status, inventory, and metrics queries.

## 9. Isolation, Least Privilege, and Auditability

Assume untrusted execution environments and enforce minimal privilege.

Implication:

- Identity-bound actions
- bounded credentials
- explicit network and resource policies
- auditable policy and runtime trails

## 10. Scale Across Solo, Community, and Enterprise Without Forking Architecture

One architecture must support:

- solo local usage
- community/shared project usage
- enterprise governance and compliance

Implication:

- deployment mode changes should adjust policy and infrastructure, not redesign
  core orchestration semantics.

## 11. Workflow Mechanics in Code, Agent Behavior in Markdown

Workflow mechanics must live in Tanren code. Command markdown defines
what an agent should do, not how Tanren resolves workflow state.

Implication:

- Issue tracker operations, verification-hook resolution, branch/PR
  flow, workflow-target selection, task-selection, gate-resolution,
  issue-provider ownership, publication workflow, and review-reply
  mechanics all belong in code.
- Shared command markdown stays provider-agnostic and repo-agnostic;
  variability lives in `{{DOUBLE_BRACE_UPPER}}` template variables
  filled install-time.
- Installed command files are rendered artifacts, not the workflow
  engine. Destructive-on-reinstall for commands; `preserve_existing`
  for repo-tailored standards.

## 12. Typed State and Single Philosophy for Artifacts

Every structured artifact the orchestrator consumes or produces —
tasks, findings, rubric scores, evidence frontmatter, phase outcomes,
events — is a typed Rust domain value with a serde schema. Markdown
checkbox parsing, `.agent-status` signal files, and ad-hoc JSON
scraping are not permitted.

Implication:

- Task state machine is monotonic (`Complete` is terminal) with typed
  guards; remediation is always a new task with provenance `origin`.
- Evidence files use YAML frontmatter + markdown body; frontmatter is
  typed and managed exclusively via tools.
- Events are the canonical history; projections are the query surface;
  `phase-events.jsonl` is the committed audit trail.

## 13. Tool-First Schema Enforcement

Agents interact with the orchestrator exclusively through a typed
tool surface. Schema validation happens at the tool boundary, not in
postflight. Invalid inputs return actionable typed errors; valid
structured state cannot be produced by any other path.

Implication:

- All structured mutation (tasks, findings, frontmatter, rubric
  scores, signposts) goes through the tool catalog. No file-writing
  shortcuts.
- Two transports share one service: MCP (primary) and CLI (fallback).
- Per-phase capability scopes confine sensitive tools
  (`escalate_to_blocker` → investigate only; `post_reply_directive`
  → handle-feedback only; `create_task` → shape-spec, investigate,
  walk-spec reject, resolve-blockers; `create_issue` → triage-audits
  and handle-feedback out-of-scope items).
- Tool call failures with `remediation` guidance keep agents
  recoverable within a session.
