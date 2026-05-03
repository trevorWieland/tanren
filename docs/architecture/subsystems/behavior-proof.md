---
schema: tanren.subsystem_architecture.v0
subsystem: behavior-proof
status: accepted
owner_command: architect-system
updated_at: 2026-05-02
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

Each behavior proof target links to one accepted behavior ID. F-0002 closes
the original "tightly coupled behavior set" exception: every `.feature`
proves exactly one behavior. Roadmap nodes that complete more than one
behavior ship one feature file per completed behavior — bundling lives at
the node level, not the feature level. The mechanical rules are documented
under "BDD Tagging And File Convention" below.

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

## BDD Tagging And File Convention

This section is the mechanical contract that
[`xtask check-bdd-tags`](../../../xtask/src/bdd_tags/) enforces and that
[`scripts/roadmap_check.py`](../../../scripts/roadmap_check.py)
cross-references. It was locked in F-0002 to close drift between
`tests/bdd/README.md`, `interfaces.md`, and three competing R-0001
attempts. Future authors should not relitigate; if the convention truly
needs to change, change it here first and then update both validators.

### File granularity

- One `.feature` file per behavior, named
  `tests/bdd/features/B-XXXX-<slug>.feature`. The slug is human-readable
  (kebab-case) and is informational only — `xtask check-bdd-tags` keys
  off the `B-XXXX` prefix.
- Multi-behavior R-* nodes (43 of 231 today, e.g. R-0007 = B-0059 +
  B-0060) ship one feature file per completed behavior. Each behavior is
  validated independently.
- A feature file's behavior must exist in `docs/behaviors/` with
  `product_status: accepted`.

### Tag rules

- **Feature-level**: exactly one tag, `@B-XXXX`, matching the filename
  prefix. No other feature-level tags.
- **Scenario-level**: exactly one of `@positive` / `@falsification`,
  plus 1–2 interface tags drawn from `@web | @api | @mcp | @cli | @tui`.
- **Closed allowlist**: the seven scenario tags above are the only tags
  permitted anywhere in the suite. `@skip`, `@wip`, `@ignore`, phase
  tags, wave tags, and proof IDs are rejected.
- **Two-interface scenarios** (e.g., create-via-CLI verify-via-web)
  require a `# rationale: <one line>` comment immediately above the
  scenario's tag block. Three or more interface tags on a single
  scenario is a hard error.

### Forbidden Gherkin constructs

- `Scenario Outline` and `Examples:` blocks are forbidden. Outlines
  generate synthetic scenario names that destabilize assessment and
  mutation IDs and break the one-witness-per-scenario rule.
- `Background:` and `Rule:` are allowed. `Rule:` is encouraged as the
  natural seam for grouping scenarios per interface inside one file.

### Coverage rules (strict equality)

A behavior is binary: fully asserted or not. There is no "partially
asserted" lane.

- The union of interface tags across the feature's scenarios must
  **equal** the behavior's frontmatter `interfaces:` set. Any tag
  outside that set is a hard error (surface drift); any frontmatter
  interface with no tagged scenario is a hard error (incomplete proof).
- For each interface in the behavior's `interfaces:` set, the feature
  must contain at least one `@positive` scenario tagged for that
  interface.
- When the R-* node's `expected_evidence.witnesses` for the behavior
  includes `falsification`, the feature must additionally contain at
  least one `@falsification` scenario tagged for **every** interface in
  the behavior's `interfaces:` set. F-0002 deliberately elevates
  falsification to per-interface coverage, stricter than the
  "where meaningful" framing in core invariant 5 above; the elevation
  is the contract because every surface needs an independent negative
  witness, not just the behavior as a whole.
- The validator already enforces that
  `expected_evidence.interfaces` equals the behavior's
  `interfaces:`; this convention rides on top.

### Validator wiring

`xtask check-bdd-tags` parses every `tests/bdd/features/**/*.feature`,
applies the rules above, and exits non-zero on any violation with a
file:line message naming the rule. It is wired into `just check` after
`check-rust-test-surface`, so every PR runs it. The inverse check —
that every `B-XXXX-*.feature` references an accepted behavior with a
DAG node — runs in `scripts/roadmap_check.py` so an orphan feature
file is caught even if the xtask validator has not been touched.

## Per-Interface BDD Wire-Harness Wiring (R-0001)

The interface tags `@web | @api | @mcp | @cli | @tui` are **witnesses,
not labels**. Each tagged scenario MUST drive the actual surface — a
real HTTP request against a spawned server for `@api` and `@mcp`, a
real subprocess for `@cli`, a real pty for `@tui`, a real browser for
`@web`. A scenario that tags `@cli` but invokes the in-process
`Handlers` facade is a tagging lie and is rejected.

This is the canonical contract that closes the per-interface BDD gap
identified during R-0001 review. It cross-references
[`profiles/rust-cargo/testing/bdd-wire-harness.md`](../../../profiles/rust-cargo/testing/bdd-wire-harness.md).

### Harness ownership

`tanren-testkit` hosts the per-feature harness traits. For the account
lifecycle the trait is `AccountHarness`; future features add their own
analogues (`SpecHarness`, `RuntimeHarness`, …) following the same
shape. Implementations:

| Tag | Implementation |
|---|---|
| `@api` | Spawns `tanren-api-app` on an ephemeral port; reqwest client with cookie jar. |
| `@cli` | `tokio::process::Command` against the compiled `tanren-cli` binary. |
| `@mcp` | Spawns `tanren-mcp-app` on an ephemeral port; `rmcp` client. |
| `@tui` | `expectrl` over `portable-pty` wrapping the compiled `tanren-tui` binary. |
| `@web` | `playwright-bdd` against a running api-app + Next.js dev server. |

Each harness exposes the same async trait surface so step bodies are
written once and dispatched by tag.

### Step dispatch

`TanrenWorld::ensure_account_ctx` (and analogues per feature) reads the
active scenario's tag set via cucumber-rs's scenario object and
instantiates the matching harness. Step bodies look up
`world.harness_mut()` and invoke the harness method; the same Gherkin
step source drives every interface.

### Single Gherkin source of truth across Rust and Playwright

The `.feature` files at `tests/bdd/features/B-XXXX-*.feature` are
consumed by:

- the Rust BDD runner (`tanren-bdd`) for `@api`/`@cli`/`@mcp`/`@tui`
  slices;
- `playwright-bdd` for the `@web` slice via a symlink at
  `apps/web/tests/bdd/features/`.

Both runners read the same Gherkin. Rust steps and Playwright fixtures
are 1:1 with each other; the matching is by step text. This prevents
the web witness from drifting from the API/CLI/MCP/TUI witnesses for
the same behavior.

### Mechanical enforcement

- `xtask check-bdd-wire-coverage` parses
  `crates/tanren-bdd/src/steps/**/*.rs` (AST via `syn`) and rejects any
  step body that calls `tanren_app_services::Handlers::` directly.
  Steps must dispatch through `*Harness` traits.
- `xtask check-deps` rejects `tanren-app-services` from
  `tanren-bdd/Cargo.toml`. The BDD crate depends on `tanren-testkit`
  and `tanren-contract` only; `Handlers` is reachable only via
  harnesses.
- The existing `xtask check-bdd-tags` continues to enforce the closed
  tag allowlist, per-interface positive coverage, and per-interface
  falsification coverage where the R-* node's
  `expected_evidence.witnesses` includes `falsification`.

## Accepted Behavior Proof Decisions

- The subsystem is named behavior proof.
- Tanren rejects a general artifact-vault architecture as the core proof
  model.
- BDD feature files are the primary executable behavior proof mechanism.
- A feature file proves exactly one accepted behavior. F-0002 closes the
  original "normally one, exceptions allowed" wording.
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
