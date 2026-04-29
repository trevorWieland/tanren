---
agent: meta
description: Tanren methodology command `identify-behaviors`
model: default
subtask: false
template: |2

  # identify-behaviors

  ## Temporary Status

  This is a temporary Tanren-method bootstrap command. It writes behavior
  artifacts directly because native behavior-catalog schemas, tools, and
  project-state events do not exist yet. Prefer structured frontmatter,
  stable IDs, explicit rationale, and small approved edits so these
  artifacts can later migrate into typed Tanren storage.

  This command is for any repository adopting the Tanren method. When it
  is used in the Tanren repository, use Tanren's local behavior docs as
  the configured catalog. Do not assume every repository has the same
  file layout.

  ## Purpose

  Turn product intent into a durable behavior canon and maintain each
  behavior's product and verification status over time.

  A behavior is a high-level user, client, operator, or runtime-actor
  capability. It describes what an actor can accomplish and what outcome
  is observable. It does not describe implementation internals.

  ## Inputs

  - Product brief, vision, motivations, constraints, and non-goals.
  - Persona, actor, interface, and concept documents.
  - Existing behavior files and behavior catalog reports.
  - BDD feature inventory and current verification docs, if present.
  - Implementation-readiness reports, if present.
  - User feedback, bug reports, client requests, audit findings, or
    planning notes supplied by the user.

  ## Editable Artifacts

  Use the repository's configured behavior-catalog location. If none is
  configured, infer the conventional location and confirm it with the
  user before editing.

  This command may:

  - create new behavior files;
  - update behavior frontmatter;
  - update product status;
  - update verification status when evidence supports it;
  - add `supersedes` links;
  - deprecate or remove behavior IDs with rationale;
  - produce a behavior catalog report.

  ## Temporary Artifact Formats

  Prefer this behavior shape:

  ```markdown
  ---
  schema: tanren.behavior.v0
  id: B-0001
  title: <imperative user-visible capability>
  area: <stable area>
  personas: []
  runtime_actors: []
  interfaces: []
  contexts: []
  product_status: draft | accepted | deprecated | removed
  verification_status: unimplemented | implemented | asserted | retired
  supersedes: []
  evidence_refs: []
  ---

  ## Intent

  ## Preconditions

  ## Observable Outcomes

  ## Out of Scope

  ## Related
  ```

  Prefer this report shape when a catalog-wide pass produces findings:

  ```markdown
  ---
  schema: tanren.behavior_catalog_report.v0
  generated_at: YYYY-MM-DD
  ---

  # Behavior Catalog Report

  ## Added
  ## Revised
  ## Status Changes
  ## Potential Gaps
  ## Needs Human Decision
  ## Needs Evidence
  ```

  ## Responsibilities

  1. Identify the repository's behavior catalog root and supporting
     persona, actor, interface, and concept docs.
  2. Read product intent and current behavior coverage before proposing
     edits.
  3. Identify missing behaviors, overlapping behaviors, oversized
     behaviors, implementation-shaped behaviors, and behaviors with stale
     status.
  4. Propose additions and revisions in a reviewable batch before
     changing files.
  5. Create behavior files for accepted additions using stable IDs.
  6. Update `product_status` only with product rationale.
  7. Update `verification_status` only with cited implementation or BDD
     evidence.
  8. Use `implemented` when code appears to support the behavior but
     active executable behavior evidence is missing.
  9. Use `asserted` only when active BDD evidence exists.
  10. Deprecate or remove accepted behavior IDs instead of silently
      repurposing them.
  11. Summarize added behaviors, revised behaviors, status changes,
      unresolved decisions, and evidence gaps.

  ## Status Rules

  - `product_status: accepted` means the product intends to support the
    behavior.
  - `verification_status: unimplemented` means no accepted code path is
    known.
  - `verification_status: implemented` means code appears to support the
    behavior, but active BDD evidence is missing.
  - `verification_status: asserted` requires cited executable behavior
    evidence.
  - `deprecated` or `removed` requires a replacement or tombstone
    rationale.

  ## Out of Scope

  - Creating roadmap DAG nodes. Use `craft-roadmap`.
  - Writing BDD scenarios directly unless the user explicitly asks for a
    combined behavior-and-evidence pass.
  - Changing implementation code.
  - Dispatching specs, creating tasks, opening pull requests, or
    mutating orchestration state.
---
