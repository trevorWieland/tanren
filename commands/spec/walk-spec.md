---
name: walk-spec
role: conversation
orchestration_loop: true
autonomy: interactive
declared_variables:
  - ISSUE_PROVIDER
  - PR_NOUN
  - READONLY_ARTIFACT_BANNER
  - SPEC_VERIFICATION_HOOK
  - TASK_TOOL_BINDING
declared_tools:
  - create_task
  - list_tasks
  - report_phase_outcome
required_capabilities:
  - task.create
  - task.read
  - phase.outcome
produces_evidence:
  - behavior-map.md
---

# walk-spec

## Purpose

The user's acceptance checkpoint. Walk through behavior outcomes live,
confirm acceptance criteria are met, surface any last concerns, and
signal completion. Tanren-code handles `{{PR_NOUN}}` creation,
roadmap updates, and `{{ISSUE_PROVIDER}}` communication after you
signal complete.

## Inputs (from your dispatch)

- The fully-implemented spec (all tasks Complete, audits passed,
  demo passed).
- The spec's `spec.md`, `plan.md`, `demo.md`, `audit.md`, and
  `behavior-map.md`.

## Responsibilities

1. Confirm prerequisites: all tasks `Complete`, `audit-spec` status
   `pass`, demo status `pass`. If not, call
   `report_phase_outcome("error", …)` immediately — walk-spec is
   not the place to fix unfinished work.
2. Run `{{SPEC_VERIFICATION_HOOK}}` and confirm green.
3. Present an implementation summary in behavior terms:
   behavior IDs, mapped scenarios, and demo evidence.
4. Walk through the demo step-by-step. For each step: explain,
   execute, show result, confirm before next.
5. If a demo step fails during the walkthrough: STOP. Call
   `create_task(title, description, origin: User)` with the
   observed failure, then `report_phase_outcome("blocked", …)`. Do not
   silently fix.
6. If the user accepts: `report_phase_outcome("complete", …)`.
   Tanren-code will create the `{{PR_NOUN}}`, update roadmap state,
   and post any required status comments.
7. If the user rejects: `create_task(origin: User)` with the user's
   concern; `report_phase_outcome("blocked", …)`.

## Verification

`{{SPEC_VERIFICATION_HOOK}}`.

## Emitting results

{{TASK_TOOL_BINDING}}

{{READONLY_ARTIFACT_BANNER}}

## Out of scope

- Creating `{{PR_NOUN}}s`
- Updating `roadmap.md`, issue comments, or any external state
- Running `audit-spec` or any other automated check
- Implementing code (if something breaks, emit a task; do not fix)
