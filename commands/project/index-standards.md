---
name: index-standards
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables:
  - STANDARDS_ROOT
  - READONLY_ARTIFACT_BANNER
  - TASK_TOOL_BINDING
declared_tools:
  - report_phase_outcome
required_capabilities:
  - standard.read
  - phase.outcome
produces_evidence:
  - updated {{STANDARDS_ROOT}}/index.yml
---

# index-standards

## Purpose

Rebuild `{{STANDARDS_ROOT}}/index.yml` from the current standards
tree. Add missing entries, remove stale entries, sort
deterministically.

## Inputs (from your dispatch)

- The current `{{STANDARDS_ROOT}}/` tree.
- The current `{{STANDARDS_ROOT}}/index.yml`.

## Responsibilities

1. Scan `{{STANDARDS_ROOT}}/` for `.md` files (excluding
   `index.yml`).
2. Diff against existing index entries.
3. For new files: parse frontmatter for `name` and `applies_to`;
   propose a description; ask the user; add to index.
4. For deleted files: remove their entries (confirm with user first).
5. Alphabetize by `(category, name)`.
6. Write the updated `index.yml`.
7. `report_phase_outcome("complete", <added/removed counts>)`.

## Out of scope

- Modifying standards content
- Running standards adherence checks

{{READONLY_ARTIFACT_BANNER}}

{{TASK_TOOL_BINDING}}
