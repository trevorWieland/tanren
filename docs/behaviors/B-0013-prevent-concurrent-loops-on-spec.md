---
schema: tanren.behavior.v0
id: B-0013
title: Prevent uncoordinated concurrent loops on the same spec
area: team-coordination
personas: [solo-builder, team-builder]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` who attempts to start ordinary implementation
work on a spec that already has active execution can discover that execution and
decide what to do next, rather than accidentally creating competing visible work
on the same spec.

## Preconditions

- The user is attempting to start an implementation loop via B-0001.
- Another active execution track already exists for the same spec.
- The start attempt is not part of an approved candidate-implementation
  comparison workflow.

## Observable outcomes

- The ordinary start attempt is blocked; no uncoordinated second loop begins.
- The user is shown the existing execution, including who owns it and its current
  state.
- From that view the user can take the actions available to them: observing
  the loop (B-0003), assisting it (B-0010), or requesting or performing a
  takeover (B-0011) — each subject to their own permissions.
- A spec whose active execution has finished or been closed can have ordinary
  implementation work started against it normally.

## Out of scope

- Queueing a start attempt so it runs after the existing loop finishes
  (autostart is covered by B-0002).
- Coordinated parallel candidate implementations, ranking, and selection
  (covered by B-0265).

## Related

- B-0001
- B-0003
- B-0010
- B-0011
- B-0265
