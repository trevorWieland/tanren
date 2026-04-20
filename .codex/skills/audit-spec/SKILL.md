---
name: audit-spec
description: Tanren methodology command `audit-spec`
role: audit
orchestration_loop: true
autonomy: autonomous
declared_variables:
- ISSUE_REF_NOUN
- PILLAR_LIST
- READONLY_ARTIFACT_BANNER
- SPEC_VERIFICATION_HOOK
- TASK_TOOL_BINDING
declared_tools:
- add_finding
- record_rubric_score
- record_non_negotiable_compliance
- list_tasks
- report_phase_outcome
required_capabilities:
- finding.add
- rubric.record
- compliance.record
- task.read
- phase.outcome
produces_evidence:
- audit.md (spec-scope narrative body)
---

# audit-spec

## Purpose

Apply the 10-pillar rubric at spec scope. Record non-negotiable
compliance. Classify findings as `fix_now` (must address in this
spec) or `defer` (backlog for future specs via `triage-audits`).

## Inputs (from your dispatch)

- The full spec folder and its accumulated diff.
- `list_tasks(filter: {spec_id})` for completion state.
- Relevant standards (for context; compliance is `adhere-spec`).
- `completeness, performance, scalability, strictness, security, stability, maintainability, extensibility, elegance, style, relevance, modularity, documentation_complete` — effective pillar set for spec scope.
- The spec's non-negotiables (from spec frontmatter).

## Responsibilities

1. Review the full spec's diff against the spec's acceptance
   criteria, non-negotiables, and pillar expectations.
2. For each finding: `add_finding` with severity, title,
   affected files/lines, source phase `audit-spec`, the pillar it
   relates to, and `attached_task` if it scopes to one. Cross-
   reference signposts to avoid duplicating known-deferred issues.
3. For each applicable pillar: `record_rubric_score(pillar, score,
   rationale, supporting_finding_ids)`. Same invariants as
   `audit-task` (target 10, passing 7, findings required for gaps,
   `fix_now` required below passing).
4. For each non-negotiable: `record_non_negotiable_compliance(name,
   status, rationale)`.
5. Write reasoning into the `audit.md` body.
6. Call `report_phase_outcome`:
   - `complete` if every pillar ≥ passing, every non-negotiable
     `pass`, demo passed, zero unaddressed `fix_now`.
   - `blocked` otherwise. Orchestrator materializes new tasks from
     `fix_now` findings; `defer` findings feed backlog curation.

## Verification

Use `just ci` if you need to ground a score by
running the spec-level gate.

## Emitting results

mcp

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
plan.md and progress.json are generated from the typed task store.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Creating `GitHub issues` for deferred items (orchestrator
  does this via `create_issue` on your classified findings)
- Editing `roadmap.md`, `plan.md`, or any orchestrator-owned file
- Creating tasks directly
- Standards compliance (that's `adhere-spec`)
- Committing, pushing, or PR mechanics
