---
name: inject-standards
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables:
  - READONLY_ARTIFACT_BANNER
  - STANDARDS_ROOT
  - TASK_TOOL_BINDING
declared_tools:
  - report_phase_outcome
required_capabilities:
  - standard.read
  - phase.outcome
produces_evidence: []
---

# inject-standards

## Purpose

Surface relevant standards into the current conversation, skill, or
plan. Two modes: **auto-suggest** (analyze context, propose relevant
standards) and **explicit** (user supplies paths).

## Inputs (from your dispatch)

- The current conversation / skill / plan context.
- User-supplied standard paths if any (explicit mode).
- `{{STANDARDS_ROOT}}/index.yml`.

## Responsibilities

1. Determine mode from the user's invocation.
2. **Auto-suggest:** read the index, match against context (files
   touched, languages, domains), propose a ranked shortlist to the
   user.
3. **Explicit:** read the supplied paths directly.
4. For each selected standard:
   - **Conversation:** print full content inline plus a three-line
     key-points summary.
   - **Skill / plan:** ask reference-vs-copy: references stay in
     sync with the source but require online lookup; copies are
     self-contained but can drift.
5. `report_phase_outcome("complete", <N standards injected>)`.

## Out of scope

- Authoring standards (that's `discover-standards`)
- Enforcing compliance (that's adherence phases)

{{READONLY_ARTIFACT_BANNER}}

{{TASK_TOOL_BINDING}}
