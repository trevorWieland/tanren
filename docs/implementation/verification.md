---
schema: tanren.implementation_verification.v0
status: current
owner_command: assess-implementation
updated_at: 2026-04-29
---

# Behavior Verification Classification

This file records the current verification state for the behavior catalog. The
behavior files themselves own product intent through `product_status`; this file
summarizes where code and executable behavior proof stand right now.

## Asserted by Active BDD

These behaviors have active BDD coverage with positive and falsification
witnesses and therefore use `verification_status: asserted`.

- `B-0068` Bootstrap Tanren assets into an existing repository
- `B-0069` Detect installer drift without mutating files
- `B-0070` Generate selected agent integrations deterministically
- `B-0071` Use the repository's installed standards
- `B-0080` See unresolved check findings that block readiness

## Implemented, Not Yet Asserted

These behaviors have public code paths or enough current implementation surface
to justify `verification_status: implemented`, but they still need dedicated
product-level BDD before they can become asserted.

- `B-0001` Start an implementation loop manually on a spec
- `B-0003` See the current state of an implementation loop
- `B-0021` See a spec's current lifecycle state
- `B-0058` Cancel a loop

## Accepted But Unimplemented

All other accepted behaviors currently use `verification_status:
unimplemented`. Some have adjacent support code, but not enough end-to-end
product behavior to claim implementation for the accepted behavior contract.

Notable adjacent-but-unimplemented areas:

- `B-0005` Respond to a question when a loop pauses on a blocker
- `B-0006` Start a walk for implementation-ready work
- `B-0014` See the history of human actions on a loop
- `B-0018` Create a draft spec manually
- `B-0049` Manage project methodology settings
- `B-0054` See outbound issues Tanren has pushed to external trackers

## Next Assertion Candidates

The next BDD additions should focus on the implemented-but-unasserted set:

- `B-0001`
- `B-0003`
- `B-0021`
- `B-0058`

Each needs at least one positive witness and one falsification witness before
its behavior doc can move to `verification_status: asserted`.
