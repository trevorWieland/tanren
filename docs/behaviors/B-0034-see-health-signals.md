---
schema: tanren.behavior.v0
id: B-0034
title: See health signals of active work
area: observation
personas: [solo-builder, team-builder, observer]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A user can see health signals about currently active work — loops erroring,
loops stalled on a stage too long, slow human response to blockers — so that
they can identify trouble spots without inspecting each loop individually.

## Preconditions

- Has visibility scope over the work being viewed.

## Observable outcomes

- The user can see which active loops are erroring or have recently errored.
- The user can see which active loops are stalled — sitting in one state
  longer than normal for that stage.
- The user can see how long it is taking humans to respond when a loop
  surfaces a blocker (B-0005), and which loops are waiting longest.
- Signals respect the scope selected via B-0037.

## Out of scope

- Automatic remediation of stalled or erroring loops.
- Defining what "normal" duration is for each stage — this behavior reports
  relative outliers, it does not require configuration.

## Related

- B-0003
- B-0005
- B-0032
- B-0037
