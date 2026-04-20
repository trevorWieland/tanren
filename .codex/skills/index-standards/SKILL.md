---
name: index-standards
description: Tanren methodology command `index-standards`
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
produces_evidence:
- updated tanren/standards/index.yml
---

# index-standards

## Purpose

Rebuild `tanren/standards/index.yml` from the current standards
tree. Add missing entries, remove stale entries, sort
deterministically.

## Inputs (from your dispatch)

- The current `tanren/standards/` tree.
- The current `tanren/standards/index.yml`.

## Responsibilities

1. Scan `tanren/standards/` for `.md` files (excluding
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

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
plan.md and progress.json are generated from the typed task store.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


mcp
