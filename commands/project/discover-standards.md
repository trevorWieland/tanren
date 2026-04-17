---
name: discover-standards
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables:
  - STANDARDS_ROOT
  - PROJECT_LANGUAGE
  - READONLY_ARTIFACT_BANNER
  - TASK_TOOL_BINDING
declared_tools:
  - report_phase_outcome
required_capabilities:
  - standard.read
  - phase.outcome
produces_evidence:
  - new standard files under {{STANDARDS_ROOT}}
---

# discover-standards

## Purpose

Interactively extract tribal knowledge from the codebase and codify
it as standards files under `{{STANDARDS_ROOT}}`. Each standard is
one rule with clear applicability metadata.

## Inputs (from your dispatch)

- Focus area (user-provided or agent-suggested).
- Representative sample of 5–10 files in that area.

## Responsibilities

1. Ask the user for a focus area, or propose 3–5 candidate areas if
   none is supplied.
2. Read representative files. Identify unusual, opinionated, or
   tribal patterns.
3. For each candidate pattern the user selects, hold a full loop:
   ask clarifying questions one at a time (no batching), draft the
   standard, confirm, write the file. Filename:
   `{{STANDARDS_ROOT}}/{category}/{kebab-slug}.md`.
4. Frontmatter required on every standard: `name`, `category`,
   `applies_to` (globs), `applies_to_languages`,
   `applies_to_domains`, `importance` (low/medium/high/critical).
5. Update `{{STANDARDS_ROOT}}/index.yml` to include the new entry.
6. `report_phase_outcome("complete", <N standards authored>)`.

## Out of scope

- Enforcing standards against the current codebase (that's adherence
  phases and `triage-audits`)
- Injecting standards into running agent context (that's
  `inject-standards`)
- Committing, pushing, or creating `{{ISSUE_REF_NOUN}}s`

{{READONLY_ARTIFACT_BANNER}}

{{TASK_TOOL_BINDING}}
