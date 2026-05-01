---
schema: tanren.behavior.v0
id: B-0005
title: Respond to a question when a loop pauses on a blocker
area: implementation-loop
personas: [solo-builder, team-builder]
interfaces: [web, api, mcp, cli, tui]
contexts: [personal, organizational]
product_status: accepted
verification_status: unimplemented
supersedes: []
---

## Intent

A `solo-builder` or `team-builder` can respond to a question surfaced by a paused
implementation loop so that the loop can resume with the answer.

## Preconditions

- A loop has paused and surfaced a question requiring human feedback.
- The user has permission to respond to the loop.

## Observable outcomes

- The question is visible alongside enough context for the user to answer it
  without leaving the interface.
- Once the user provides an answer, the loop resumes and continues from where
  it paused.
- If the user cannot or chooses not to answer, the loop remains paused and
  stays visible in that state.

## Out of scope

- Automatic routing of questions to the "right" person.
- Multi-participant discussion threads on a single question.

## Related

- B-0003
- B-0004
