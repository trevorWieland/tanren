---
schema: tanren.experience_proof_matrix.v0
status: draft
owner_command: design-experience
updated_at: 2026-05-05
---

# Experience Proof Matrix

This projection maps project surfaces to proof adapter expectations. It is the
surface-native companion to behavior proof: BDD remains the behavior-level proof
language, while each surface decides what evidence makes that proof observable.

| Surface | Kind | Primary Evidence | Supporting Evidence |
|---------|------|------------------|---------------------|
| `web` | `responsive_gui` | Playwright BDD over the browser surface | Storybook component states, axe checks, responsive screenshots |
| `api` | `machine_contract` | Contract BDD against HTTP requests and responses | OpenAPI schema generation, machine-readable error cases |
| `mcp` | `agent_tool_contract` | MCP tool contract scenarios | Tool visibility, permission-boundary, and structured-error cases |
| `cli` | `command_line` | Process execution with stdout, stderr, and exit-code assertions | Golden transcripts and structured JSON output checks |
| `tui` | `terminal_ui` | PTY-driven interaction scenarios | Screen snapshots, resize checks, and keyboard navigation cases |

Future adopting projects may replace or extend these rows with game replays,
desktop automation, mobile device runs, chat transcripts, SDK examples, or
embedded-device captures.
