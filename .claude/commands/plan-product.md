---
name: plan-product
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables:
- ISSUE_REF_NOUN
- PRODUCT_ROOT
- READONLY_ARTIFACT_BANNER
- TASK_TOOL_BINDING
declared_tools:
- report_phase_outcome
required_capabilities:
- phase.outcome
produces_evidence:
- tanren/product/mission.md
- tanren/product/roadmap.md
- tanren/product/tech-stack.md
---

# plan-product

## Purpose

Establish foundational product docs for a new project. This is
one-shot scaffolding, not recurring work.

## Inputs (from your dispatch)

- Any existing `tanren/product/` state (may be empty).
- Templates from `templates/product/`.

## Responsibilities

1. Detect existing docs. If all three exist, ask the user whether
   to regenerate or edit.
2. Ask about problem, target users, solution.
3. Ask about MVP features and post-launch features.
4. Ask about tech stack, OR use an existing tech-stack standard if
   one is installed.
5. Generate `tanren/product/mission.md`,
   `tanren/product/roadmap.md`, `tanren/product/tech-stack.md`
   from templates, populated with the user's answers.
6. `report_phase_outcome("complete", <files created>)`.

## Out of scope

- Creating `GitHub issues` for roadmap items (shape-spec does
  that when the user picks one up)
- Modifying standards (that's `discover-standards`)
- Running any gate or audit

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
spec.md, plan.md, tasks.md, tasks.json, demo.md, and progress.json
are generated from the typed event stream.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


Use Tanren MCP tools for all structured mutations (for example `create_task`, `add_finding`, `report_phase_outcome`). CLI fallback uses the same contract:
`tanren methodology --phase <phase> --spec-id <spec_uuid> --spec-folder <spec_dir> <noun> <verb> --json '<payload>'`.
