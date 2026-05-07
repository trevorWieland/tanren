---
schema: tanren.experience_proof_matrix.v0
status: draft
owner_command: design-experience
updated_at: 2026-05-07
---

# Experience Proof Matrix

This projection maps project surfaces to proof adapter expectations. It is the
surface-native companion to behavior proof: BDD remains the behavior-level proof
language, while each surface decides what evidence makes that proof observable.

| Surface | Kind | Primary Evidence | Supporting Evidence | `surfaces.yml` `proof:` |
|---------|------|------------------|---------------------|--------------------------|
| `web` | `responsive_gui` | Playwright BDD over the browser surface | Storybook component states, axe checks, responsive screenshots | `playwright_bdd`, `storybook_component_state`, `accessibility_scan`, `responsive_screenshot` |
| `api` | `machine_contract` | Contract BDD against HTTP requests and responses | OpenAPI schema generation, machine-readable error cases | `contract_bdd`, `openapi_schema`, `machine_readable_error_case` |
| `mcp` | `agent_tool_contract` | MCP tool contract scenarios | Tool visibility, permission-boundary, and structured-error cases | `mcp_tool_contract`, `tool_visibility_case`, `permission_boundary_case` |
| `cli` | `command_line` | Process execution with stdout, stderr, and exit-code assertions | Golden transcripts and structured JSON output checks | `process_execution`, `golden_transcript`, `exit_code_case` |
| `tui` | `terminal_ui` | PTY-driven interaction scenarios | Screen snapshots, resize checks, and keyboard navigation cases | `pty_session`, `screen_snapshot`, `keyboard_navigation_case` |

The right-hand column matches the `proof:` field for each surface in
`docs/experience/surfaces.yml`. Drift between this table and that file is a
real defect for `B-0292` (reject-surface-registry-drift) to detect.

Implementation status of each adapter (scaffold vs. wired vs. not yet built)
is owned by `assess-implementation`, not by this projection. This file
declares **what evidence each surface owes**; whether the harness exists
yet is a separate question tracked in implementation assessment.

Future adopting projects may replace or extend these rows with game replays,
desktop automation, mobile device runs, chat transcripts, SDK examples, or
embedded-device captures.
