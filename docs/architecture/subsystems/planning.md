---
schema: tanren.subsystem_architecture.v0
subsystem: planning
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Planning Architecture

## Purpose

This document defines Tanren's planning subsystem. Planning is the product
method layer that turns a project idea into accepted product intent, behavior
contracts, architecture decisions, and a spec-sized roadmap DAG.

Planning is generic to projects built with Tanren. Tanren's own repository is
one project that uses this method, but its personas, interfaces, behaviors,
runtime actors, and architecture records are not universal nouns for every
project.

## Subsystem Boundary

The planning subsystem owns:

- product brief and project identity records;
- project-defined planning vocabularies;
- target users, problems, constraints, non-goals, and success signals;
- behavior catalog schema and behavior lifecycle;
- architecture record lifecycle;
- roadmap DAG shape, synthesis rules, proposal, and acceptance;
- planning proposals and accepted planning changes;
- decision memory, assumptions, rejected alternatives, and conflict
  resolution;
- planning document and machine-readable repo projections;
- planning history and planning revision recovery.

The planning subsystem does not own active spec execution, task state, worker
assignment, runtime environments, proof execution, implementation assessment
execution, bug triage execution, proactive analysis execution, PR state, merge
state, or release operations. Those subsystems may feed planning through
proposals, proof results, and source signals, but planning owns the accepted
direction.

## Core Invariants

1. **Planning is project-scoped.** A Tanren project is exactly one repository,
   and product planning canon is scoped to that project.
2. **The planning method is strict; the project vocabulary is open.** Tanren
   enforces typed schemas, traceability, lifecycle, and coverage rules while
   allowing each project to define its own personas, surfaces, contexts,
   actor classes, and domain vocabulary.
3. **Behavior is the unit of product meaning.** Roadmap nodes, specs, demos,
   BDD proof, release learning, and bug triage must trace back to accepted
   behavior contracts.
4. **Behavior acceptance is separate from verification.** A behavior describes
   product intent. Implementation, proof, assertion, and coverage are separate
   behavior-proof, assessment, or read-model state.
5. **Bootstrapping order is strict.** A first project planning pass follows
   `plan-product -> identify-behaviors -> architect-system -> craft-roadmap`.
6. **Subsequent planning can revisit earlier layers.** After bootstrap, an
   authorized planning change may revise behaviors, architecture, or roadmap
   directly when upstream context is already sufficient.
7. **Roadmap executable nodes are spec-sized.** A roadmap node should be small
   enough to shape into one spec, execute, prove, walk, and review.
8. **Executable work completes behavior.** Each executable roadmap node must
   complete at least one accepted behavior unless explicitly marked as a
   temporary bootstrap exception.
9. **Foundation exceptions are temporary.** Infrastructure-only work that
   cannot yet complete a behavior must declare why it is unavoidable, what
   behavior it enables, and when the exception should disappear.
10. **Planning documents are projections.** Product docs, behavior files,
    architecture docs, and roadmap views are generated or validated
    projections from typed planning events.

## Planning Method Funnels

Tanren has three distinct project-method funnels. They share state,
interfaces, events, MCP access, and human-agent interaction patterns, but they
serve different purposes.

### Primary Planning Funnel

The primary planning funnel is the frontloaded path for turning a project idea
into a buildable plan:

```text
plan-product
-> identify-behaviors
-> architect-system
-> craft-roadmap
```

This funnel is normally led by a product owner, founder, lead engineer, or
other actor accountable for product direction. It creates accepted planning
state and repo projections.

### Alteration Funnel

The alteration funnel handles changes discovered after initial planning:

```text
assessment | bug triage | feedback | proactive analysis | release learning
-> planning proposal
-> accepted planning change
-> roadmap revision
```

This funnel lets support, field engineering, customer success, operators,
automated analysis, and post-ship learning influence planning without silently
owning product direction. Alteration sources produce proposed changes by
default. Accepted changes require the configured authority or approval.

### Delivery Funnel

The delivery funnel executes one accepted roadmap node:

```text
shape-spec
-> orchestration
-> walk-spec
-> review / merge / release
```

Planning informs delivery through accepted behaviors, roadmap nodes,
architecture constraints, and decisions. Delivery returns proof results,
outcomes, bugs, findings, and release learning that may feed the alteration
funnel.

## Project Planning Vocabulary

Tanren provides strict planning schemas with project-defined controlled
vocabularies.

Each project may define allowed values for concepts such as:

- personas or user segments;
- observable product surfaces or interfaces;
- contexts or deployment/use environments;
- external client classes;
- system or runtime actor classes;
- product areas;
- behavior tags;
- domain-specific concepts.

Tanren must not hard-code its own project personas or public interfaces into
all projects. A library, CLI, API service, mobile app, game, internal platform,
and Tanren itself may each define different observable surfaces.

The method still requires enough structure for typed events, validation,
roadmap coverage, proof routing, and projection generation. For example, a
behavior should identify who or what experiences the behavior and through
which observable surface it can be demonstrated or asserted. A project that
seems to have no interface should still define the package, protocol, command,
API, UI, runtime, or operator surface through which the behavior is observed.

## Product Brief

The product brief records why the project exists and what success means.

Product brief state includes:

- project identity;
- target users or user segments;
- user problems;
- motivations;
- constraints;
- non-goals;
- success signals;
- assumptions;
- open decisions.

The product brief is accepted planning state. Drafts, proposals, comments, and
alternatives are separate records until accepted. Repo markdown is a
projection, not canonical truth.

## Behavior Catalog

The behavior catalog records what users, operators, clients, packages,
protocol consumers, runtime actors, or other project-defined subjects can do.

Behavior records include:

- stable behavior ID;
- title;
- project-defined area;
- relevant project-defined actors, personas, or subjects;
- relevant observable surfaces;
- context or environment metadata where useful;
- product lifecycle status;
- intent;
- preconditions;
- observable outcomes;
- out-of-scope boundaries;
- related behavior IDs.

Behavior records do not own implementation verification status. Verification,
assertion, proof coverage, implementation support, and proof links are
separate state derived from implementation assessment, BDD proof,
behavior-proof state, and delivery outcomes.

Behavior lifecycle values are product-state values such as draft, accepted,
deprecated, removed, or superseded. Projects may add structured metadata, but
accepted behavior IDs remain stable for traceability.

## Architecture Records

Architecture records are planning state that explain how the accepted product
intent and behavior catalog should be implemented.

Architecture records include:

- system boundary and invariants;
- technology posture;
- delivery model;
- operations model;
- subsystem designs;
- accepted decisions;
- rejected alternatives;
- open questions;
- constraints that affect roadmap synthesis.

Architecture is part of the primary planning funnel. Initial roadmap creation
requires enough accepted architecture to make implementation order meaningful.
Later behavior changes may or may not require architecture changes. When they
do, architecture updates flow through planning proposals or direct accepted
changes according to policy.

## Roadmap DAG

The roadmap DAG is the accepted graph of spec-sized work needed to deliver the
project's accepted behavior catalog.

Roadmap node rules:

- executable nodes are spec-sized;
- each executable node completes at least one accepted behavior;
- dependencies are explicit;
- sequencing rationale is recorded;
- node scope is narrow enough for one shaped spec;
- expected proof, demo, or assertion obligations are visible enough to shape
  the spec later;
- risks, assumptions, and architectural prerequisites are linked;
- grouping structures such as initiatives, milestones, themes, and releases
  are rollups or projections, not executable nodes.

Foundation-only nodes are permitted only as temporary bootstrap exceptions.
They must identify the accepted behavior they enable, why no behavior can be
completed yet, and what later node removes the exception.

## Roadmap Creation

`craft-roadmap` synthesizes a roadmap DAG from accepted planning state.

Inputs include:

- accepted product brief;
- accepted behavior catalog;
- accepted architecture records;
- constraints and non-goals;
- decision memory and rejected alternatives;
- optional implementation assessment for existing codebases;
- known findings, feedback, or release outcomes when replanning.

Roadmap synthesis should:

- group behaviors into spec-sized increments;
- choose dependencies that preserve correctness and product meaning;
- reduce architectural and product risk early;
- prefer user-visible or proof-visible progress;
- surface missing behavior or architecture questions as planning proposals;
- avoid inventing hidden assumptions;
- distinguish required foundation work from behavior-completing work;
- produce a reviewable roadmap proposal before acceptance when policy requires
  approval.

Accepted roadmap changes emit typed events. Roadmap documents, graphs, and
coverage views are projections.

## Implementation Assessment Input

Implementation assessment is not the primary planning funnel. It is an input
to planning and replanning.

Assessment inspects current implementation, tests, docs, proof results, and runtime
behavior to report what appears true. It may identify accepted behaviors that
look implemented, asserted, missing, stale, regressed, or uncertain. Those
classifications do not change behavior acceptance. They produce read models,
findings, and planning proposals.

Initial roadmap creation for a new project may not need assessment. Replanning
an existing codebase should use assessment where current implementation reality
matters.

## Planning Proposals

Planning proposals are structured suggested changes to accepted planning state.

Proposal targets include:

- product brief;
- project vocabulary;
- behavior catalog;
- architecture records;
- roadmap DAG;
- assumptions;
- rejected alternatives;
- decision records;
- standards or configuration where the change affects planning direction.

Proposals may be accepted, rejected, revised, superseded, or left open.
Acceptance emits typed planning events and updates projections. Policy decides
when a qualified actor may directly accept a change versus when review or
approval is required.

The alteration funnel produces proposals by default. A customer-facing or
field actor can say that a client needs a new integration, that a provider
adapter broke, or that a behavior is misaligned without silently rewriting
product direction.

## Intake And Alteration Sources

Planning accepts inputs from:

- customer feedback;
- bug reports;
- meeting notes;
- external tickets;
- audit findings;
- mutation test results;
- security reviews;
- benchmarks;
- proactive analysis;
- support or field engineering reports;
- post-release health and feedback.

Bug and issue intake should classify the report before changing roadmap state.
Core classifications include:

- false alarm or out of scope;
- missing accepted behavior;
- accepted behavior misaligned with user intent;
- behavior implemented but lacking proper executable assertion;
- behavior asserted but currently regressed.

Tanren should not add work to the roadmap DAG until the planning meaning is
known. Intake may create an intake item, a draft spec, or a planning proposal
depending on the source and policy, but accepted roadmap changes remain typed
planning state.

## Decision Memory

Planning records why decisions were made, not only what the current projection
says.

Decision memory includes:

- accepted decisions;
- assumptions;
- rejected alternatives;
- stale assumptions;
- unresolved disagreements;
- conflicts between product direction, architecture, proof signals, and feedback;
- rationale for roadmap sequencing and deferrals.

Rejected alternatives are first-class records because they prevent repeated
debate and preserve tradeoffs for future planning.

## Recovery

Planning recovery is event-sourced and non-destructive.

Supported recovery patterns include:

- restore a previous planning revision by appending corrective events;
- supersede a bad behavior, architecture decision, or roadmap node;
- route landed-work regret into follow-up work;
- mark assumptions stale when new proof or source signals invalidate them;
- preserve history while changing accepted direction.

Tanren does not delete planning history to hide bad decisions. It records the
correction and preserves the reason.

## Repo Projections

Planning projections make canonical planning state readable in the repository.

Common projections include:

- product vision and concept documents;
- behavior catalog files;
- architecture records;
- roadmap graphs and coverage views;
- decision records and rejected alternatives;
- machine-readable planning indexes.

Repo projections are generated or validated from typed planning events. Manual
edits to Tanren-owned planning projections are drift and are handled by the
state subsystem.

## Accepted Planning Decisions

- Planning is project-scoped.
- The first project bootstrap follows
  `plan-product -> identify-behaviors -> architect-system -> craft-roadmap`.
- Subsequent planning may revisit layers out of order when accepted context is
  already sufficient.
- Project-defined planning vocabularies are allowed and expected.
- Tanren enforces method schemas and traceability without hard-coding one
  project's personas, interfaces, or actors into every project.
- Behavior records own product intent, not verification status.
- Verification, assertion, implementation status, and proof coverage are
  separate state or read models.
- Architecture records are a planning record family.
- The roadmap DAG is part of planning.
- Executable roadmap nodes are spec-sized and complete at least one accepted
  behavior.
- Foundation-only roadmap nodes are temporary bootstrap exceptions.
- Alteration sources produce planning proposals by default.
- Intake is classified before accepted roadmap mutation.
- Repo planning documents are projections, not source of truth.
- Universal behavior schema includes ID, title, area, actor, interfaces,
  outcome statement, context, positive examples, falsification examples,
  non-goals, dependencies, lifecycle state, and project-defined vocabulary
  fields.
- Foundation-work exceptions are allowed only during first-project bootstrap
  for repository connection, stack setup, baseline proof harness, and initial
  projection generation. They expire when the first behavior-backed roadmap
  node is accepted.
- Approval is required by default for accepted behavior changes, roadmap DAG
  changes, architecture record changes, standards changes, provider/policy
  changes, and any proposal that creates executable spec work.
- Assessment classifications remain in assessment read models. Planning read
  models may consume summarized behavior coverage and route proposals but do
  not own assertion state.
- The stable roadmap DAG schema includes node ID, title, behavior links,
  dependencies, size, state, acceptance policy, foundation exception metadata,
  owner scope, rationale, and source event positions.

## Rejected Alternatives

- **Hard-coding Tanren's own interfaces or personas into all projects.**
  Rejected because Tanren must plan arbitrary products, not only itself.
- **Behavior records owning verification status.** Rejected because behavior
  acceptance is product intent, while verification is implementation and proof
  state.
- **Roadmap nodes as broad initiatives.** Rejected because executable work must
  be small enough to shape, execute, prove, walk, and review.
- **Silent roadmap mutation from intake.** Rejected because bugs, feedback,
  and analysis need classification before they alter accepted work.
- **Architecture after roadmap by default.** Rejected for first bootstrap
  because roadmap sequencing depends on system and technology decisions.
- **Deleting planning history to recover.** Rejected because planning recovery
  should preserve proof/source signals, rationale, and accountability.
