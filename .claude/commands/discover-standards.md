---
name: discover-standards
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables:
- ISSUE_REF_NOUN
- READONLY_ARTIFACT_BANNER
- STANDARDS_ROOT
- TASK_TOOL_BINDING
declared_tools:
- report_phase_outcome
required_capabilities:
- standard.read
- phase.outcome
produces_evidence:
- new standard files under tanren/standards
---

# discover-standards

## Purpose

Interactively extract tribal knowledge from the codebase and codify
it as standards files under `tanren/standards`. Each standard is
one rule with clear applicability metadata. Prefer behavior-first
standards where testing strategy is in scope.

## Inputs (from your dispatch)

- Focus area (user-provided or agent-suggested).
- Representative sample of 5–10 files in that area.

## Responsibilities

1. Ask the user for a focus area, or propose 3–5 candidate areas if
   none is supplied.
2. Read representative files. Identify unusual, opinionated, or
   tribal patterns.
3. If testing standards are in scope, explicitly probe for BDD
   requirements:
   - behavior inventory expectations
   - scenario traceability expectations
   - mutation and coverage interpretation policies
4. For each candidate pattern the user selects, hold a full loop:
   ask clarifying questions one at a time (no batching), draft the
   standard, confirm, write the file. Filename:
   `tanren/standards/{category}/{kebab-slug}.md`.
5. Frontmatter required on every standard: `name`, `category`,
   `applies_to` (globs), `applies_to_languages`,
   `applies_to_domains`, `importance` (low/medium/high/critical).
6. Update `tanren/standards/index.yml` to include the new entry.
7. `report_phase_outcome("complete", <N standards authored>)`.

## Out of scope

- Enforcing standards against the current codebase (that's adherence
  phases)
- Injecting standards into running agent context (that's
  `inject-standards`)
- Committing, pushing, or creating `GitHub issues`

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
spec.md, plan.md, tasks.md, tasks.json, demo.md, audit.md,
signposts.md, progress.json, and .tanren-projection-checkpoint.json
are generated from the typed event stream.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


Use Tanren MCP tools for all structured mutations in this phase.
MCP-first canonical invocation set for phase `discover-standards`:
The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.
- MCP `report_phase_outcome` payload: `{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}`
- CLI `report_phase_outcome` command: `"$TANREN_CLI" --database-url "$TANREN_DATABASE_URL" methodology --methodology-config "$TANREN_CONFIG" --phase discover-standards --spec-id <spec_uuid> --spec-folder "$TANREN_SPEC_FOLDER" phase outcome --json '{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}'`
