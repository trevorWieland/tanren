---
agent: meta
description: Tanren methodology command `inject-standards`
model: default
subtask: false
template: |2

  # inject-standards

  ## Purpose

  Surface relevant standards into the current conversation, skill, or
  plan. Two modes: **auto-suggest** (analyze context, propose relevant
  standards) and **explicit** (user supplies paths).

  ## Inputs (from your dispatch)

  - The current conversation / skill / plan context.
  - User-supplied standard paths if any (explicit mode).
  - `tanren/standards/index.yml`.

  ## Responsibilities

  1. Determine mode from the user's invocation.
  2. **Auto-suggest:** read the index, match against context (files
     touched, languages, domains), propose a ranked shortlist to the
     user.
  3. **Explicit:** read the supplied paths directly.
  4. For each selected standard:
     - **Conversation:** print full content inline plus a three-line
       key-points summary.
     - **Skill / plan:** ask reference-vs-copy: references stay in
       sync with the source but require online lookup; copies are
       self-contained but can drift.
  5. `report_phase_outcome("complete", <N standards injected>)`.

  ## Out of scope

  - Authoring standards (that's `discover-standards`)
  - Enforcing compliance (that's adherence phases)

  ⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
  plan.md and progress.json are generated from the typed task store.
  Postflight reverts unauthorized edits and emits an
  UnauthorizedArtifactEdit event. Use the typed tool surface
  (MCP or CLI) to record progress.


  mcp
---
