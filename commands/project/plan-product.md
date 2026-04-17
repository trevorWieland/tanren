---
name: plan-product
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables:
  - PRODUCT_ROOT
  - READONLY_ARTIFACT_BANNER
  - TASK_TOOL_BINDING
declared_tools:
  - report_phase_outcome
required_capabilities:
  - phase.outcome
produces_evidence:
  - {{PRODUCT_ROOT}}/mission.md
  - {{PRODUCT_ROOT}}/roadmap.md
  - {{PRODUCT_ROOT}}/tech-stack.md
---

# plan-product

## Purpose

Establish foundational product docs for a new project. This is
one-shot scaffolding, not recurring work.

## Inputs (from your dispatch)

- Any existing `{{PRODUCT_ROOT}}/` state (may be empty).
- Templates from `templates/product/`.

## Responsibilities

1. Detect existing docs. If all three exist, ask the user whether
   to regenerate or edit.
2. Ask about problem, target users, solution.
3. Ask about MVP features and post-launch features.
4. Ask about tech stack, OR use an existing tech-stack standard if
   one is installed.
5. Generate `{{PRODUCT_ROOT}}/mission.md`,
   `{{PRODUCT_ROOT}}/roadmap.md`, `{{PRODUCT_ROOT}}/tech-stack.md`
   from templates, populated with the user's answers.
6. `report_phase_outcome("complete", <files created>)`.

## Out of scope

- Creating `{{ISSUE_REF_NOUN}}s` for roadmap items (shape-spec does
  that when the user picks one up)
- Modifying standards (that's `discover-standards`)
- Running any gate or audit

{{READONLY_ARTIFACT_BANNER}}

{{TASK_TOOL_BINDING}}
