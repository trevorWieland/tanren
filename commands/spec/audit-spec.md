---
name: audit-spec
role: audit
orchestration_loop: true
autonomy: autonomous
declared_variables:
  - ISSUE_REF_NOUN
  - PILLAR_LIST
  - READONLY_ARTIFACT_BANNER
  - TASK_TOOL_BINDING
declared_tools:
  - add_finding
  - list_findings
  - resolve_finding
  - record_finding_still_open
  - defer_finding
  - supersede_finding
  - record_rubric_score
  - record_non_negotiable_compliance
  - list_tasks
  - report_phase_outcome
required_capabilities:
  - finding.add
  - finding.read
  - finding.lifecycle
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
spec) or `defer` (backlog for future specs through project intake).

## Inputs (from your dispatch)

- The full spec folder and its accumulated diff.
- `list_tasks(filter: {spec_id})` for completion state.
- Relevant standards (for context; compliance is `adhere-spec`).
- `{{PILLAR_LIST}}` — effective pillar set for spec scope.
- The spec's non-negotiables (from spec frontmatter).
- Projected spec/task artifacts, linked scenarios, and demo outcomes.

## Responsibilities

1. Review the full spec's diff against the spec's acceptance
   criteria, non-negotiables, and pillar expectations.
2. Verify behavior coverage integrity at spec scope:
   - every shaped behavior is mapped
   - mapped scenarios exist and pass
   - deprecated behaviors are intentionally handled
3. Verify mutation evidence quality at spec scope:
   surviving mutants are triaged and mapped to concrete follow-up
   actions when needed.
4. Verify coverage-gap interpretation quality:
   uncovered paths are classified as missing scenario vs dead/non-
   scenario support code.
5. Call `list_findings(status: open, severity: fix_now, scope:
   spec, check_kind: audit)` and recheck existing audit blockers.
6. Resolve fixed prior blockers with `resolve_finding`; record
   persistent ones with `record_finding_still_open`; defer or
   supersede only with durable evidence.
7. For each new finding: `add_finding` with severity, title,
   affected files/lines, source phase `audit-spec`, the pillar it
   relates to, and `attached_task` if it scopes to one. Cross-
   reference signposts to avoid duplicating known-deferred issues.
8. For each applicable pillar: `record_rubric_score(pillar, score,
   rationale, supporting_finding_ids)`. Same invariants as
   `audit-task` (target 10, passing 7, findings required for gaps,
   `fix_now` required below passing).
9. For each non-negotiable: `record_non_negotiable_compliance(name,
   status, rationale)`. Use a stable short slug for `name`; put the
   full non-negotiable text and evidence in `rationale`.
10. Write reasoning into the `audit.md` body.
11. Call `report_phase_outcome`:
    - `complete` if every pillar ≥ passing, every non-negotiable
    `pass`, demo passed, zero open blocking audit `fix_now`.
    - `blocked` otherwise. Orchestrator dispatches `investigate` for
    autonomous remediation and then resumes the loop.

## Verification

Use existing spec-gate evidence from the projected artifacts and event
history. Do not rerun the repository gate from this command; the
orchestrator owns the spec gate.

## Emitting results

{{TASK_TOOL_BINDING}}

{{READONLY_ARTIFACT_BANNER}}

## Out of scope

- Creating `{{ISSUE_REF_NOUN}}s` for deferred items (orchestrator
  does this via `create_issue` on your classified findings)
- Editing `ROADMAP.md`, `plan.md`, or any orchestrator-owned file
- Creating tasks directly
- Standards compliance (that's `adhere-spec`)
- Committing, pushing, or PR mechanics
