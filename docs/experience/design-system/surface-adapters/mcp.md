---
schema: tanren.design_surface_adapter.v0
surface_kind: agent_tool_contract
status: draft
owner_command: define-design-system
updated_at: 2026-05-07
---

# MCP Adapter

The MCP adapter projects shared design-system records into tool names, argument
schemas, structured results, permission boundaries, and tool-call traces.

## Projection

- Vocabulary terms map to structured tool errors.
- Pattern IDs map to reusable tool result and recovery shapes.
- Tool names follow `tanren.<area>.<verb>` unless a shaped spec records an
  accepted deviation.
- Permission denial is explicit and structured, not represented by hiding a
  tool that otherwise exists for the project.

## Required Evidence

- MCP tool contract proof.
- Tool visibility and permission-boundary cases.
- Tool-call traces for walk evidence.

## Drift Signals

- Tool errors that do not use the shared error taxonomy.
- Tool-call output that cannot be traced to a behavior and pattern ID.
- File-system side effects outside Tanren state unless a surface contract
  explicitly allows them.
