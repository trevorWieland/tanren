---
schema: tanren.implementation_readiness_view.v0
source: readiness.json
status: current
owner_command: assess-implementation
updated_at: 2026-04-30
---

# Implementation Readiness

This file is the human-readable projection of current behavior implementation
readiness. The machine-readable source is `readiness.json`.

The current assessment is static analysis only. It did not run `just tests`,
`just check`, or `just ci`. It summarizes the completed readiness aggregate from
`artifacts/behavior/readiness/runs/20260430T151940Z`, generated at
`2026-04-30T17:07:49Z`.

## Catalog Snapshot

| Source | Count |
|---|---:|
| Accepted behaviors in catalog | 284 |
| Asserted behaviors excluded from readiness scan | 5 |
| Accepted behaviors assessed in `readiness.json` | 279 |
| Completed per-behavior reports | 279 |

The embedded report statuses in `readiness.json` were normalized on
2026-04-30 so `verification_status` again matches the behavior catalog
frontmatter. Recommended verification statuses were preserved as assessment
recommendations.

## Readiness Classification

| Readiness status | Count | Meaning |
|---|---:|---|
| `already_implemented` | 2 | Static evidence indicates the behavior exists end to end, but assertion may still be missing. |
| `close_needs_work` | 11 | A coherent implementation surface exists, with bounded product or interface gaps remaining. |
| `partial_foundation` | 237 | Adjacent primitives exist, but meaningful behavior work remains. |
| `not_started` | 29 | Little or no direct implementation surface was found. |

The current implementation is therefore broad but shallow. Most accepted
behaviors have useful primitives nearby, especially methodology services,
store projections, domain events, policy scaffolding, and command markdown, but
they are not complete product behaviors across the declared public surfaces.

## Near-Term Implementation Surface

These behaviors have the strongest static evidence and should be treated as the
nearest assertion or completion candidates.

| Behavior | Current verification | Readiness | Assessment recommendation |
|---|---|---|---|
| `B-0058` Cancel a loop | `implemented` | `close_needs_work` | keep `implemented`; add assertion proof |
| `B-0073` Accept walked work | `unimplemented` | `close_needs_work` | promote after bounded completion |
| `B-0076` Define acceptance criteria for a spec | `unimplemented` | `already_implemented` | promote after catalog review and proof |
| `B-0078` Shape a draft spec for prioritization | `unimplemented` | `already_implemented` | promote after catalog review and proof |
| `B-0157` Explain why a spec is not ready | `unimplemented` | `close_needs_work` | promote after bounded completion |
| `B-0252` Preserve worker output without leaking secrets | `unimplemented` | `close_needs_work` | promote after bounded completion |

Several existing `implemented` behaviors are still assessed as
`close_needs_work`: `B-0001`, `B-0003`, and `B-0021`. The dominant reason is
not absence of backend primitives; it is incomplete interface coverage, missing
behavior proof, and unresolved alignment with readiness/dependency gates.

## Architecture Alignment

| Architecture alignment | Count |
|---|---:|
| `aligned` | 177 |
| `divergent` | 12 |
| `unclear` | 90 |

The highest-impact architecture gap is the interface contract. The accepted
interface architecture requires all public clients to use the HTTP control
plane and equivalent operations to reach the same application service
(`docs/architecture/subsystems/interfaces.md:44`). Current code does not meet
that shape:

- `bin/tanren-api/src/main.rs:13` is an empty API binary.
- `bin/tanren-tui/src/main.rs:9` explicitly says the TUI is a deferred stub.
- `bin/tanren-mcp/src/tool_registry.rs:101` exposes a methodology tool registry,
  but many accepted behaviors need broader product, control-plane, runtime,
  integration, and observation tools.

This means many backend-adjacent behaviors remain incomplete even when CLI or
service-layer code exists.

## Roadmap Gaps

The assessment points to these roadmap inputs:

- Build the HTTP API control plane before claiming broad cross-interface
  behavior completion.
- Wire CLI, MCP, TUI, and eventual web surfaces through the shared API contract
  instead of direct local service paths.
- Add behavior proof for the close and already-implemented candidates. Active
  scenarios must carry exactly one behavior ID and one witness tag per
  `tests/bdd/README.md:5`.
- Separate real product completion from temporary command markdown. Many
  methodology commands describe the intended method, but native schemas, typed
  events, read models, and first-party interfaces are still incomplete.
- Treat `partial_foundation` as sequencing evidence, not as completion. These
  behaviors usually need explicit contracts, events/projections, interface
  commands, and positive plus falsification BDD.

