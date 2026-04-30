---
schema: tanren.implementation_verification.v0
status: current
owner_command: assess-implementation
updated_at: 2026-04-30
---

# Behavior Verification Classification

This file records the current verification state for the accepted behavior
catalog. Behavior files own product intent through `product_status`; this file
summarizes where code and executable behavior proof stand right now.

This projection is static-analysis output. No verification command was run for
this assessment.

## Catalog Verification State

| Verification status | Count |
|---|---:|
| `asserted` | 5 |
| `implemented` | 4 |
| `unimplemented` | 275 |
| Accepted total | 284 |

## Asserted By Active BDD

These behaviors have active behavior proof and use
`verification_status: asserted`.

- `B-0068` Bootstrap Tanren assets into an existing repository
- `B-0069` Detect installer drift without mutating files
- `B-0070` Generate selected agent integrations deterministically
- `B-0071` Use the repository's installed standards
- `B-0080` See unresolved check findings that block readiness

## Implemented, Not Yet Asserted

These behaviors currently use `verification_status: implemented` in the
behavior catalog, but they still need dedicated product-level proof before they
can become asserted.

- `B-0001` Start an implementation loop manually on a spec
- `B-0003` See the current state of an implementation loop
- `B-0021` See a spec's current lifecycle state
- `B-0058` Cancel a loop

Static readiness assessment recommends keeping `B-0058` implemented. It
recommends demoting or reworking `B-0001`, `B-0003`, and `B-0021` unless the
missing interface, gating, and proof gaps are intentionally accepted as
temporary limitations.

## Promotion Candidates

The readiness aggregate recommends `implemented` for these currently
unimplemented behaviors:

- `B-0073` Accept walked work
- `B-0076` Define acceptance criteria for a spec
- `B-0078` Shape a draft spec for prioritization
- `B-0157` Explain why a spec is not ready
- `B-0252` Preserve worker output without leaking secrets

Do not change the behavior files from this projection alone. Each promotion
needs a focused review against the behavior contract, then positive and
falsification BDD before assertion.

## Unclear Recommendations

Three behaviors have an `unknown` recommended verification status in the
readiness aggregate and should be manually reviewed before roadmap synthesis:

- `B-0065` See existing members' access to an organization
- `B-0115` Approve a generated plan when policy requires approval
- `B-0278` Classify bug reports against behavior status

## Verification Gaps

The largest verification gap is not isolated to a few behaviors. The readiness
aggregate repeatedly points to missing BDD under `tests/bdd/features/`,
incomplete API/TUI binaries, a narrow MCP methodology surface, and missing
first-class contracts or projections for many product behaviors.

Roadmap synthesis should prioritize proof for the close candidates first, then
turn the partial foundations into typed product surfaces before attempting broad
status promotion.

