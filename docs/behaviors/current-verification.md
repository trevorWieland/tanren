# Behavior Verification Classification

This file records the BDD revamp cutover classification for the existing
behavior catalog. It prevents the retained installer proof from duplicating
broader draft product behaviors.

## Already proven by active BDD

None of `B-0001` through `B-0067` is already proven by the retained BDD suite.
The removed Phase 0 suite used `BEH-P0-*` proof IDs and mostly exercised
synthetic models rather than product-facing catalog behaviors.

## Testable now, not included in this cutover

- `B-0001` Start an implementation loop manually on a spec
- `B-0003` See the current state of an implementation loop
- `B-0021` See a spec's current lifecycle state
- `B-0058` Cancel a loop

These have public code paths, but they need dedicated product-level scenarios
after the behavior framework is stable.

## Partially implemented, not accepted

- `B-0005` Respond to a question when a loop pauses on a blocker
- `B-0006` Trigger a walk to review a completed loop
- `B-0014` See the history of human actions on a loop
- `B-0018` Create a spec, optionally from an external ticket
- `B-0049` Manage project-tier configuration
- `B-0054` See outbound issues Tanren has pushed to external trackers

These remain draft because current code has supporting pieces but not enough
end-to-end product behavior to claim acceptance.

## Planned or aspirational

All remaining `B-0001` through `B-0067` behaviors remain draft.

`B-0025` remains draft even though installer behavior is adjacent: connecting a
repository as an account project is not the same capability as bootstrapping
local Tanren assets.

`B-0049` remains draft even though `tanren.yml` exists: installer-managed local
config is not the full project-tier configuration management behavior.

