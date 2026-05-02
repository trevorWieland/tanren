---
schema: tanren.subsystem_architecture.v0
subsystem: behavior-proof
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Behavior Proof Architecture

## Purpose

This document defines Tanren's behavior proof architecture. Behavior proof is
the executable bridge between accepted product behavior and durable proof that
the implementation actually demonstrates that behavior.

The subsystem is intentionally narrow. Tanren does not need a general artifact
vault. It needs behavior-linked executable proof, written at the product
behavior level, with enough structure for assessment, roadmap coverage, walks,
and regression analysis.

## Subsystem Boundary

The behavior proof subsystem owns:

- behavior-to-proof linkage;
- BDD feature and scenario expectations;
- positive and falsification witness requirements;
- assertion support for accepted behaviors;
- proof-result read models consumed by assessment;
- behavior-proof coverage analysis;
- mutation testing interpretation as proof-quality feedback;
- proof staleness inputs;
- rules distinguishing behavior proof from implementation-detail tests.

The subsystem does not own generic blob storage, raw log retention, CI artifact
archiving, implementation assessment, PR gate enforcement, orchestration
state, runtime execution, or planning acceptance. Other subsystems may produce
signals, but behavior proof decides what counts as executable demonstration of
an accepted behavior.

## Core Invariants

1. **Behavior proof is tied to behavior IDs.** Executable proof exists to show
   accepted behavior, not incidental implementation details.
2. **Asserted behavior requires executable proof.** A behavior can be assessed
   as asserted only when current behavior proof satisfies project policy.
3. **Proof does not live in behavior records.** Behavior records describe
   product intent. Proof files, proof runs, assertion state, and coverage are
   separate state and projections.
4. **BDD is the primary proof form.** `.feature` files and behavior-level
   scenarios are the standard way Tanren proves accepted behavior.
5. **Positive and falsification witnesses matter.** A proof should show both
   the desired behavior and a meaningful invalid, blocked, or negative case
   where applicable.
6. **Behavior proof is observable.** Tests should exercise the same observable
   surface through which the behavior is experienced or consumed.
7. **Unit tests are not behavior proof by default.** They can support
   implementation support, but assertion requires behavior-level proof.
8. **Coverage is a planning signal.** When tests are behavior-linked, low
   coverage points to missing behaviors, irrelevant code, or proof that fails
   to exercise the real behavior path.
9. **Mutation testing evaluates proof strength.** Mutation survivors are
   assessment signals that BDD proof may not be meaningfully asserting the
   behavior.
10. **Proof results can become stale.** Code, behavior, architecture,
   dependency, configuration, or runtime changes can make prior proof only
   potentially current.

## Behavior-To-Proof Linkage

Each behavior proof target links to one accepted behavior ID. A feature file
should normally prove one behavior. Exceptions are allowed only when a single
observable scenario necessarily demonstrates a tightly coupled behavior set,
and the linkage must remain explicit.

Proof linkage records:

- behavior ID;
- proof file or proof target;
- project-defined observable surface;
- positive witness scenarios;
- falsification witness scenarios where applicable;
- latest run status;
- latest run time;
- proof freshness or staleness status;
- mutation-quality status where available.

This linkage lets Tanren answer which accepted behaviors are unproven,
asserted, stale, regressed, or missing meaningful negative coverage.

## BDD Feature Model

BDD features are executable product proof. They describe behavior in terms a
user, operator, client, package consumer, or runtime actor can observe.

Feature rules:

- a feature cites the behavior ID it proves;
- scenarios use product vocabulary, not crate, table, or implementation
  details;
- scenarios exercise public or observable surfaces;
- scenarios include positive witnesses;
- scenarios include falsification witnesses where meaningful;
- scenario names and steps remain stable enough for assessment and reporting;
- proof failures report behavior-level failure meaning.

BDD steps may use implementation helpers underneath, but the scenario should
remain behavior-oriented. A test that only proves a helper function or internal
branch is not behavior proof.

## Positive And Falsification Witnesses

A positive witness shows the desired behavior succeeds.

A falsification witness shows that a meaningful invalid, unauthorized,
blocked, malformed, conflicting, or negative case is rejected or handled as the
behavior requires.

Falsification witnesses are required where they are meaningful because they
prove Tanren is not merely checking that the happy path executes. Exceptions
must be explicit and rare, for example when a behavior is purely informational
and no meaningful falsification case exists.

## Assertion Support

Behavior proof supports assessment classification. It does not directly mutate
behavior acceptance.

Assessment may classify a behavior as asserted when:

- the behavior is accepted;
- required proof targets exist;
- positive witnesses pass;
- required falsification witnesses pass;
- proof is not known stale under current policy;
- mutation or proof-quality signals do not invalidate assertion where such
  signals are required.

Assertion state is a read model or assessment result. It is not stored inside
the behavior record.

## Coverage Interpretation

Tanren's preferred test coverage signal comes from behavior-linked tests.

When behavior proof is the only or primary high-level test suite, coverage has
product meaning:

- uncovered code may indicate missing intended behavior;
- uncovered code may be irrelevant or dead implementation;
- uncovered code may be implementation infrastructure that needs a behavior
  path to exercise it;
- behavior proof may be too shallow to exercise the real flow;
- additional behavior contracts may need to be identified.

Coverage is not treated as a standalone quality target. It is a diagnostic
signal for behavior gaps, proof gaps, and implementation relevance.

## Mutation Testing

Mutation testing is proof-quality assessment. It asks whether behavior proof
would fail if meaningful implementation logic were broken.

Mutation testing is too expensive to run as a normal PR gate. It runs as a
nightly job against the main branch (or any longer-lived integration branch)
when the source has changed since the last run. The nightly job uploads
mutation reports as failure artifacts so that subsequent PRs can address the
surviving mutants. Mutation testing is intentionally NOT part of `just ci` and
must not gate merges. Results feed the assessment subsystem and quality
controls; they do not directly fail active specs unless policy explicitly
routes them that way.

Mutation survivors can indicate:

- a behavior proof does not assert the outcome it claims;
- a falsification witness is missing;
- an accepted behavior is underspecified;
- implementation code is irrelevant to accepted behavior;
- the mutation is equivalent or not meaningful.

Mutation results do not automatically change accepted behavior or fail active
specs unless policy explicitly routes them that way.

## Relationship To Orchestration

Orchestration consumes behavior proof obligations when shaping and completing
specs.

A shaped spec should identify which accepted behaviors it completes and which
behavior proof must be added or updated. Completion should include passing
behavior proof for the completed behavior unless the spec is an explicit
temporary bootstrap exception.

Orchestration owns when proof runs during a spec and how failed proof routes
back into task work. Behavior proof owns what kind of proof is meaningful.

## Relationship To Assessment

Assessment consumes proof results and proof-quality signals.

Assessment uses behavior proof to classify behaviors as implemented, asserted,
missing, stale, regressed, or uncertain. Assessment also interprets mutation
testing, coverage, stale proof, and bug reports against behavior proof state.

Behavior proof supplies structured proof status. Assessment decides current
provenance and routing.

## Relationship To Planning

Planning owns accepted behavior. Behavior proof owns executable assertion of
that behavior.

When a behavior cannot be proven naturally, that is a planning signal. It may
mean the behavior is too vague, the observable surface is unclear, the
falsification case is missing, or the project needs a different behavior
definition.

Roadmap nodes should be sized so their completion can add or update behavior
proof for at least one accepted behavior.

## Proof Projections

Repo-local proof files and proof summaries are projections from typed proof
state and project files.

Common projections include:

- BDD `.feature` files;
- proof indexes that map behavior IDs to feature files;
- proof run summaries;
- behavior assertion coverage views;
- mutation-quality summaries tied to behavior proof.

Tanren-owned proof projections follow the state subsystem's projection and
drift rules. User-authored test implementation code remains normal project
code, but Tanren-owned proof indexes and generated summaries are controlled
projections.

## Audit And Events

Behavior proof state is event-sourced where Tanren owns the record.

Events include:

- proof target created, updated, deprecated, or removed;
- proof linked or unlinked from behavior;
- proof run started, completed, failed, or marked inconclusive;
- positive witness passed or failed;
- falsification witness passed, failed, or waived with rationale;
- proof marked potentially stale;
- mutation-quality signal recorded;
- behavior assertion support changed.

Proof events do not store secret values or raw runtime logs. They store
behavior-level proof results, metadata, and references needed for assessment.

## Accepted Behavior Proof Decisions

- The subsystem is named behavior proof.
- Tanren rejects a general artifact-vault architecture as the core proof
  model.
- BDD feature files are the primary executable behavior proof mechanism.
- A feature file should normally prove one accepted behavior.
- Asserted behavior requires active behavior-level proof.
- Behavior records do not store verification or assertion status.
- Positive witnesses are required for behavior assertion.
- Falsification witnesses are required where meaningful.
- Unit tests do not count as behavior proof by default.
- Coverage from behavior-linked tests is interpreted as a behavior/proof gap
  signal, not a standalone target.
- Mutation testing is proof-quality assessment and usually belongs outside
  normal PR gating.
- Assessment owns current assertion classification; behavior proof owns proof
  status and proof-quality signals.
- Universal feature metadata includes behavior ID, witness kind, proof target,
  interface or surface under test, fixture scope, assertion policy version,
  source event position, and redaction class.
- Falsification-witness exceptions are limited to behavior-not-testable,
  external-system-unavailable, destructive-real-world-action, and
  policy-prohibited-observation, each requiring rationale and review.
- Mutation result categories are killed, survived, timed-out, equivalent,
  invalid, skipped-by-policy, and infrastructure-failed.
- Tanren owns proof projections, feature indexes, proof status, and proof
  policy metadata. Ordinary project test code remains project-owned.
- Proof is stale when relevant behavior intent, dependencies, runtime policy,
  source code, configuration, or proof policy changes after the supporting
  proof event.

## Rejected Alternatives

- **General artifact vault.** Rejected because Tanren's core need is
  behavior-linked executable proof, not broad blob retention.
- **Behavior files storing proof status.** Rejected because behavior files
  express product intent while proof status is implementation and assessment
  state.
- **Unit tests as default behavior proof.** Rejected because implementation
  detail tests do not necessarily demonstrate observable product behavior.
- **Coverage as a standalone success metric.** Rejected because coverage is
  useful only when interpreted against behavior-linked proof.
- **Mutation testing as a mandatory PR gate.** Rejected because mutation
  testing is often too expensive and better suited to assessment unless a
  project explicitly configures otherwise.
