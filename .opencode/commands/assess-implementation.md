---
agent: meta
description: Tanren methodology command `assess-implementation`
model: default
subtask: false
template: |2

  # assess-implementation

  ## Temporary Status

  This is a temporary Tanren-method bootstrap command. It writes implementation
  state projections directly because native implementation-assessment schemas,
  typed tools, and project-method events do not exist yet. Prefer structured JSON
  and concise markdown summaries so these artifacts can later migrate into typed
  Tanren storage.

  This command is for any repository adopting the Tanren method. Use the
  repository's configured implementation artifact paths; if none are configured,
  use the conventional `docs/implementation/` paths.

  ## Purpose

  Assess what is currently true about accepted behavior implementation and
  verification. This command does not decide product intent, choose architecture,
  or create roadmap nodes. It reports current state so `craft-roadmap` can plan
  from evidence.

  ## Inputs

  - Product projections from `docs/product/**`.
  - Accepted behavior catalog from `docs/behaviors/**`.
  - Architecture projections from `docs/architecture/**`.
  - Current code, tests, BDD features, command docs, configuration, and existing
    implementation reports.
  - Static analysis or readiness outputs, including temporary reports under
    `artifacts/behavior/readiness/`.

  ## Editable Artifacts

  This command owns:

  - `docs/implementation/readiness.json`
  - `docs/implementation/readiness.md`
  - `docs/implementation/verification.md`

  Temporary run logs, prompts, per-behavior partial reports, and progress markers
  belong under `artifacts/`, not in `docs/`.

  ## Responsibilities

  1. Operate in static-analysis mode unless the user explicitly authorizes a
     verification run.
  2. Read behavior files, implementation surfaces, BDD evidence, docs, and
     architecture before classifying readiness.
  3. Preserve the distinction between `product_status` and
     `verification_status`.
  4. Classify accepted behaviors as asserted, implemented without assertion,
     close, partially founded, not started, stale, contradictory, or unclear.
  5. Cite implementation, test, and documentation evidence by path and line where
     possible.
  6. Do not update behavior files unless the user explicitly requests a combined
     behavior-status pass.
  7. Write or refresh machine-readable readiness JSON and concise human
     projections.
  8. Summarize gaps that should feed roadmap synthesis.

  ## Out of Scope

  - Editing product docs. Use `plan-product`.
  - Editing behavior docs or product status. Use `identify-behaviors`.
  - Choosing or revising architecture. Use `architect-system`.
  - Creating roadmap DAG nodes. Use `craft-roadmap`.
  - Writing implementation code.
  - Dispatching specs, creating tasks, opening pull requests, or mutating
    orchestration state.
---
