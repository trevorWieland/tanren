---
schema: tanren.behavior.v0
id: B-0289
title: MCP join tool redacts session token from debug output
area: governance
personas: [solo-builder, team-builder]
interfaces: [mcp]
contexts: [organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

The MCP `account.join_organization` tool accepts a bearer session token as
input. The token must never appear in `Debug` output, preventing accidental
leakage through tracing, logging, or developer inspection.

## Preconditions

- The MCP surface is reachable and authenticated with a valid API key.
- A valid session token and invitation token are available.

## Observable outcomes

- The `JoinToolRequest` struct's `Debug` implementation prints
  `<redacted>` in place of the raw `session_token` value.
- The tool still successfully converts the session token into a
  `SessionToken` for store lookup and completes the join operation.

## Out of scope

- Redaction of invitation tokens (they are identifiers, not secrets).
- Other MCP tools that do not carry bearer session tokens.

## Related

- B-0045
- B-0043
