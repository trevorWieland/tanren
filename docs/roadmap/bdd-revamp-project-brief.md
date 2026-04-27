# BDD Revamp Project Brief

## Purpose

Tanren needs a first-party behavior verification system that is repo-wide,
phase-agnostic, easy for developers and AI agents to navigate, and strong
enough to drive CI decisions. The current BDD, mutation, and coverage setup
proved a useful concept, but it hardcodes a temporary Phase 0 proof model into
the test architecture. That makes new behavior tests easy to miss, makes
coverage and mutation signals misleading, and disconnects executable behavior
proof from the canonical behavior catalog in `docs/behaviors`.

This brief describes the desired design direction. It is intentionally
implementation-light. A fresh agent should use this as the product and system
intent, then inspect the repository to determine the exact code, test,
documentation, and CI changes needed.

## Current Problems

The current system has three separate concepts that should be unified:

- `docs/behaviors/B-XXXX-*.md`: user-visible product behavior catalog.
- `docs/roadmap/proof/phase0/bdd.md` and `behavior-traceability.json`: a
  phase-specific proof inventory using `BEH-P0-*` IDs.
- `tests/bdd/phase0/*.feature` plus `crates/tanren-bdd-phase0`: executable
  Cucumber scenarios tied to the Phase 0 proof model.

This creates several bad properties:

- BDD identity is phase-bound instead of product-bound.
- The active executable scenarios use `BEH-P0-*` IDs instead of the stable
  `B-XXXX` behavior IDs.
- Feature files live under `tests/bdd/phase0`, which encodes when behavior was
  introduced rather than what behavior the product guarantees.
- The BDD runner crate is named `tanren-bdd-phase0`, which makes a temporary
  implementation phase part of the permanent test architecture.
- `just tests` hardcodes the phase0 BDD source, traceability file, feature file
  list, BDD crate name, mutation script, and coverage script.
- Mutation is driven by `scripts/proof/phase0/run_mutation_stage.sh`, which
  mutates selected BDD harness files rather than treating product crates as the
  primary mutation target.
- Coverage is driven by `scripts/proof/phase0/run_coverage_stage.sh`, which
  runs through the phase0 BDD runner and then classifies workspace source with
  phase/wave-specific assumptions.
- `xtask` contains hardcoded constants for `tanren-bdd-phase0`,
  `docs/roadmap/proof/phase0`, `BEH-P0-*`, and wave-to-source mappings.
- The current BDD crate contains some real end-to-end tests, especially for
  installer behavior, but also contains synthetic in-memory behavior models.
  Those synthetic models can pass without proving the real product behavior.

The net effect is that adding a new feature or new BDD module can accidentally
fall outside mutation, coverage, traceability, or CI classification. The system
also makes it hard for a developer or agent to answer basic questions:

- Which product behaviors are accepted?
- Which accepted behaviors have executable proof?
- Which behavior tests exercise this source file?
- Which source files are not reachable from any behavior?
- When a spec changes code, which behavior docs and feature files should change?

## Design Goal

Create a behavior verification system with these properties:

- First-party: BDD is a normal repo test surface, not a temporary proof script.
- Repo-wide: tests, coverage, and mutation apply to all behavior scenarios and
  relevant product code by default.
- Phase-agnostic: phases may exist in roadmap docs, but test structure and
  behavior IDs must not encode phase names.
- Behavior-ID driven: stable `B-XXXX` IDs from `docs/behaviors` are the only
  behavior identity used by executable scenarios.
- Discoverable: developers and agents can browse behavior docs and feature
  files as a product behavior library.
- Generated traceability: traceability is derived from behavior docs, feature
  tags, BDD execution, coverage, and mutation artifacts rather than maintained
  by hand.
- Real execution: BDD steps should exercise real product boundaries wherever
  practical, especially CLI, MCP, app-service, storage, and installer flows.
- CI-simple: local and CI flows use high-level commands with clear artifacts.
- Actionable gaps: coverage and mutation reports should point to missing
  behavior, dead code, weak scenarios, or orphaned product code.

## Source Of Truth Model

Use distinct artifacts with explicit responsibilities.

### Behavior Docs

`docs/behaviors` is the product behavior source of truth.

Each behavior file describes what a user can do, why it matters, preconditions,
observable outcomes, out-of-scope boundaries, and related behavior IDs.

Behavior docs must remain product-facing. They should not become test scripts,
step-by-step implementation recipes, or test fixture descriptions.

The stable behavior ID format is `B-XXXX`.

### Feature Files

`tests/bdd/features` should contain executable examples that prove behavior
docs. Feature files are tests and should remain in the test tree.

Feature files should not duplicate the full prose from behavior docs. They
should reference behavior IDs with tags and express concrete executable
examples.

Example:

```gherkin
@behavior @installer @cli
Feature: Bootstrap Tanren into an existing repository

  @B-0025 @positive
  Scenario: Empty repository can be bootstrapped with a standards profile
    Given an empty target repository
    When the user runs tanren install with profile "rust-cargo"
    Then the repository is ready to use with Tanren

  @B-0025 @falsification
  Scenario: Unknown standards profile is rejected without partial writes
    Given an empty target repository
    When the user runs tanren install with profile "missing-profile"
    Then installation fails validation
    And no bootstrap files are written
```

### Generated Traceability

Do not preserve hand-authored traceability JSON as a source of truth. Generate
traceability from:

- behavior doc frontmatter,
- Gherkin feature/scenario tags,
- BDD execution results,
- coverage reports,
- mutation reports.

Generated artifacts should be written under `artifacts/behavior`, `artifacts/bdd`,
`artifacts/coverage`, and `artifacts/mutation`, with a stable `latest` symlink
or equivalent.

## Linking Rules

The link between behavior docs and feature files should be strict and
mechanically enforced.

Required rules:

- Every accepted behavior must have at least one passing `@positive` scenario.
- Every accepted behavior must have at least one passing `@falsification`
  scenario.
- Every behavior-owning scenario must reference exactly one `@B-XXXX` tag.
- Every `@B-XXXX` tag must resolve to exactly one behavior file.
- Every scenario that references a behavior must also declare exactly one
  witness tag: `@positive` or `@falsification`.
- Draft behaviors may exist without executable scenarios.
- Deprecated behaviors must not be referenced by active scenarios unless the
  scenario is explicitly tagged as deprecation, compatibility, or migration
  coverage.
- Skipped, ignored, pending, or work-in-progress scenarios must be forbidden in
  the required behavior suite.
- Phase tags such as `@phase0`, wave tags such as `@wave_a`, and proof IDs such
  as `BEH-P0-*` must be retired from active behavior tests.

Recommended rules:

- A scenario should normally map to one behavior. If a single end-to-end flow
  exercises many behaviors, keep it as a workflow scenario but require separate
  focused scenarios for each behavior's positive and falsification witnesses.
- Feature-level tags should describe domain and interface, such as
  `@installer`, `@methodology`, `@cli`, `@mcp`, `@runtime`, or `@storage`.
- Scenario titles should be readable user/product examples, not implementation
  claims.

## File Organization

Recommended structure:

```text
docs/behaviors/
  README.md
  project-setup/
    B-0025-connect-existing-repo.md
    B-0026-create-new-project.md
  methodology/
    B-0068-bootstrap-tanren-repo.md

tests/bdd/
  README.md
  features/
    project-setup/
      connect-existing-repo.feature
    methodology/
      bootstrap-install.feature
    runtime/
      spec-loop.feature

crates/tanren-bdd/
  Cargo.toml
  src/main.rs
  src/world.rs
  src/steps/
    installer.rs
    methodology.rs
    runtime.rs
    assertions.rs

crates/tanren-testkit/
  Cargo.toml
  src/temp_repo.rs
  src/process.rs
  src/fixtures.rs
  src/assertions.rs
```

It is acceptable to keep `docs/behaviors` flat at first if moving behavior docs
would create too much churn. The important requirement is that behavior IDs are
globally unique and mechanically parsed.

Recommended product areas for categorization:

- `project-setup`
- `methodology`
- `installer`
- `spec-lifecycle`
- `task-lifecycle`
- `runtime`
- `permissions`
- `configuration`
- `observation`
- `external-trackers`
- `agent-integrations`

## Behavior Metadata

Behavior frontmatter should support discovery and validation.

Recommended fields:

```yaml
---
id: B-0025
title: Connect Tanren to an existing repository
status: accepted
capability_area: project-setup
personas: [solo-dev, team-dev]
interfaces: [cli, mcp]
contexts: [personal, organizational]
risk: high
supersedes: []
introduced_by: []
---
```

Required minimum:

- `id`
- `title`
- `status`
- `personas`
- `interfaces`
- `contexts`
- `supersedes`

Recommended validation:

- File name must start with the behavior ID.
- Frontmatter ID must match the file name ID.
- `status` must be one of `draft`, `accepted`, or `deprecated`.
- `personas` must reference `docs/behaviors/personas.md`.
- `interfaces` must reference `docs/behaviors/interfaces.md`.
- `capability_area`, if present, must be one of the known product areas.
- `supersedes` entries must reference existing behavior IDs.
- Deprecated behavior files must include replacement or rationale.

## BDD Runner Design

Replace `tanren-bdd-phase0` with a phase-agnostic BDD runner, likely
`tanren-bdd`.

The runner should support high-level commands such as:

```text
cargo run -p tanren-bdd -- run --features tests/bdd/features
cargo run -p tanren-bdd -- list
cargo run -p tanren-bdd -- validate
cargo run -p tanren-bdd -- report --json artifacts/bdd/latest/run.json
```

The exact CLI is flexible, but the system should provide:

- feature discovery by directory,
- execution of all behavior scenarios by default,
- fail-on-skipped behavior,
- structured run artifacts,
- stable scenario identity,
- behavior ID extraction,
- positive/falsification witness extraction,
- no hardcoded feature list.

The runner should favor real product execution:

- CLI behavior should run real `tanren-cli` binaries.
- MCP behavior should run the real MCP server contract where practical.
- Installer behavior should run direct binaries, not `cargo run` inside step
  definitions.
- Service behavior should use public app-service boundaries or API boundaries.
- Storage behavior should use real store implementations and migrations when
  feasible.

Synthetic in-memory models should be avoided for accepted product behavior.
They may be useful only for testing the BDD harness itself or for deliberately
isolated parser/validator behavior.

## Test Binary Strategy

BDD tests that exercise installed command behavior should use the installed
Tanren CLI from `PATH`. Proof-driver scripts must not accept alternate binary
paths or target workspace builds such as `target/debug/tanren-cli`; those paths
exercise a different contract than a real user or orchestrator sees.

Build-and-run patterns such as `cargo run` remain inappropriate inside step
definitions because they are slower and make coverage/mutation attribution
harder. When behavior requires the public command surface, install the current
CLI first and call `tanren-cli` directly.

## Coverage Design

Coverage should answer two questions:

1. Which accepted behaviors have passing executable witnesses?
2. Which product code is actually reached by behavior tests?

Coverage should not be treated as behavior proof by itself. Low coverage is an
excellent gap signal, but it needs classification.

Expected classifications:

- `covered_behavior`: accepted behavior has passing positive and falsification
  scenarios.
- `missing_positive_witness`: accepted behavior has no passing positive
  scenario.
- `missing_falsification_witness`: accepted behavior has no passing
  falsification scenario.
- `uncovered_product_code`: product source not reached by behavior tests.
- `unowned_product_code`: product source has no known behavior ownership.
- `dead_or_obsolete_candidate`: source appears unreachable and has no behavior
  owner.
- `support_code_covered`: test/support code that is measured but not a product
  behavior owner.

The coverage flow should be owned by a high-level command, probably in `xtask`,
not by a phase shell script.

Example:

```text
cargo run -p tanren-xtask -- behavior coverage
```

or as part of:

```text
cargo run -p tanren-xtask -- behavior verify
```

Coverage implementation needs special care for CLI and MCP subprocesses. The
goal is for behavior coverage to include product binaries executed by scenarios,
not just the BDD runner process. The implementation should inspect
`cargo-llvm-cov` support for instrumented subprocess execution and profile
merging before finalizing the design.

Coverage reports should make gaps actionable:

```text
Uncovered product code:
- bin/tanren-cli/src/commands/install.rs
  likely area: installer
  related behaviors: B-0025, B-0068
  recommendation: add falsification scenarios for config conflict handling

- crates/tanren-app-services/src/methodology/standards.rs
  likely area: methodology
  related behaviors: none
  recommendation: document behavior or remove/deprecate code
```

## Mutation Design

Mutation should test product behavior strength, not primarily the BDD harness.

The current mutation path mutates selected files in `tanren-bdd-phase0`. That
is not the desired final contract. The desired system should mutate product
crates and use the BDD suite as the test command.

The mutation flow should be owned by a high-level command, probably in `xtask`:

```text
cargo run -p tanren-xtask -- behavior mutation
```

Mutation should:

- discover workspace packages from `cargo metadata`,
- exclude test-only crates such as `tanren-bdd`, `tanren-testkit`, and `xtask`,
- include product crates and binaries by default,
- avoid hardcoded file lists,
- use the full BDD suite as the test command,
- shard automatically in CI if needed,
- emit structured artifacts linked to `B-XXXX` behavior IDs where possible.

Surviving mutants should be triaged as:

- weak or missing BDD scenario,
- missing falsification coverage,
- equivalent mutant,
- dead code candidate,
- unreachable code path requiring behavior decision,
- test harness limitation.

Mutation may need pragmatic staging because full workspace mutation can be
expensive. Staging is acceptable only if the policy is explicit and discovery
is automatic. Do not encode temporary phase/wave/source-file assumptions.

## Behavior Impact At Spec Time

Every new spec should include a behavior impact section. The goal is to make
behavior documentation and BDD updates part of feature design, not cleanup.

Recommended spec section:

```md
## Behavior Impact

Existing behaviors modified:
- B-0025 Connect Tanren to an existing repository

New behaviors added:
- B-0068 Bootstrap Tanren into a fresh repository

Behaviors deprecated:
- None

BDD evidence required:
- Positive: fresh repo install with default agents
- Falsification: invalid profile fails before writes
- Falsification: conflicting existing config fails
```

Future tooling should support commands such as:

```text
tanren behavior assess --spec path/to/spec.md
tanren behavior impact --changed-files
tanren behavior gaps
tanren behavior inventory
```

These commands should help developers and agents answer:

- Which behaviors does this spec create, modify, or remove?
- Are accepted behavior docs updated?
- Are matching feature scenarios present?
- Are positive and falsification witnesses present?
- Do changed product files map to existing behavior areas?
- Did coverage identify changed code without behavior execution?

## Behavior Modification And Removal Policy

Behavior IDs are stable product contracts. Do not delete or reuse IDs casually.

Policy:

- If the capability still exists and only wording changes, modify the behavior
  doc and keep the same ID.
- If a behavior is replaced, mark the old behavior `deprecated`, create or
  update the successor, and set `supersedes`.
- If a behavior was never real or product direction changed, mark it
  `deprecated` with rationale.
- Do not delete behavior files as normal cleanup. Tombstones preserve
  traceability.
- Active feature files must not reference deprecated behaviors unless they are
  testing compatibility, migration, or deprecation behavior.
- Removing a scenario should require proof that no accepted behavior depends on
  it, or that another scenario now satisfies the required witness.

## CI And Developer Flow

The developer-facing commands should stay simple:

```text
just check
just tests
just ci
```

`just tests` should be a high-level behavior verification command, not a list
of phase-specific shell invocations.

Desired logical flow:

```text
behavior inventory validation
BDD feature discovery and execution
behavior coverage classification
behavior mutation triage
artifact generation
```

This could be implemented as:

```text
cargo run -p tanren-xtask -- behavior verify
```

CI should upload stable artifacts:

```text
artifacts/behavior/enforced/latest
artifacts/bdd/enforced/latest
artifacts/coverage/enforced/latest
artifacts/mutation/enforced/latest
```

Avoid artifact names that encode a temporary phase.

## Minimum Actionable Scope

The first implementation pass should focus on establishing the new contract
without trying to perfect every report.

Minimum deliverables:

1. Introduce a phase-agnostic BDD crate or rename/migrate the existing one to
   `tanren-bdd`.
2. Move active feature files from `tests/bdd/phase0` to
   `tests/bdd/features` or an equivalent phase-agnostic structure.
3. Replace `BEH-P0-*` tags with real `B-XXXX` behavior IDs from
   `docs/behaviors`, adding or refining behavior docs as needed.
4. Replace hardcoded feature lists with feature discovery.
5. Replace the manual phase traceability source with generated validation from
   behavior docs and feature tags.
6. Add validation rules for behavior IDs, witness tags, skipped scenarios, and
   accepted behavior coverage obligations.
7. Update `just tests` to call the new high-level behavior path.
8. Make mutation discovery automatic enough that new BDD modules and product
   code cannot silently fall out of scope.
9. Make coverage discovery automatic enough that new BDD modules and feature
   files cannot silently fall out of scope.
10. Retire or quarantine `scripts/proof/phase0` from active CI.
11. Update docs so `docs/behaviors` is clearly the source of product behavior
    truth and `tests/bdd/features` is the executable evidence layer.

## Future Extensions

After the minimum system is in place, extend it with:

- behavior-to-source ownership maps,
- changed-file behavior impact analysis,
- behavior gap recommendations from low coverage,
- dead code candidate reports,
- richer mutation survivor classification,
- generated HTML or Markdown behavior library,
- per-interface behavior matrices,
- per-persona behavior matrices,
- spec-time behavior impact validation,
- automatic PR comments summarizing behavior coverage changes,
- trend reports for accepted behaviors, missing witnesses, and unowned product
  code.

## Risk Areas

### Treating Coverage As Behavior Proof

Coverage is a signal, not the behavior contract. High coverage can still miss
important falsification behavior. Low coverage can indicate missing behaviors,
dead code, or support code. The system must classify gaps rather than treating
line percentage as the final truth.

### Mutating The Test Harness Instead Of Product Code

Mutation should primarily test whether behavior scenarios catch product-code
regressions. Mutating BDD runner code can be useful for harness tests, but it
must not be the main mutation confidence signal.

### Recreating Manual Traceability

Manual traceability JSON is likely to drift. Prefer generated traceability from
behavior docs, feature tags, execution results, coverage, and mutation outputs.

### Overloading Feature Files With Product Prose

Feature files should be readable executable examples, not duplicated behavior
documents. The behavior docs explain the product contract. Feature scenarios
prove it.

### Keeping Phase Concepts In Active Test Identity

Roadmap phases are planning context. They should not determine crate names,
feature paths, behavior IDs, mutation source lists, or coverage classification.

### Synthetic Step Definitions

Synthetic worlds can produce false confidence. Accepted product behavior should
exercise real product boundaries as much as practical.

### Subprocess Coverage

CLI and MCP tests often execute child processes. Coverage will be misleading
unless the implementation intentionally captures coverage for those binaries.
Investigate this before claiming repo-wide behavior coverage.

## Acceptance Criteria For The Revamp

The revamp is successful when:

- There is no active `tanren-bdd-phase0` test crate.
- Active BDD feature files do not live under a phase-named directory.
- Active scenarios use `B-XXXX` IDs from `docs/behaviors`.
- `just tests` discovers all behavior features automatically.
- Adding a new feature file automatically includes it in BDD execution,
  coverage, and behavior validation.
- Adding a new BDD step module cannot silently fall out of mutation or coverage
  accounting because the system no longer hardcodes step source files.
- Accepted behaviors missing positive or falsification witnesses fail
  validation.
- Unknown behavior IDs in feature tags fail validation.
- Deprecated behavior IDs in active scenarios fail validation unless explicitly
  marked as migration/deprecation coverage.
- Coverage artifacts identify behavior coverage and uncovered product code.
- Mutation artifacts are generated from product-code mutation with full BDD as
  the test command, or have a clearly staged automatic policy that does not
  encode phases.
- CI artifact paths and report names are behavior-level, not phase-level.
- Documentation clearly explains how behavior docs, feature files, coverage,
  mutation, specs, and CI interact.

## Guidance For The Implementing Agent

Start by reading these areas:

- `docs/behaviors`
- `tests/bdd`
- `crates/tanren-bdd-phase0`
- `xtask/src/main.rs`
- `justfile`
- `.github/workflows/rust-ci.yml`
- `scripts/proof/phase0`
- current coverage and mutation artifacts under `artifacts`

Do not treat the current phase0 proof structure as the desired architecture.
Treat it as migration input.

Prefer a design that makes the correct thing automatic. Avoid dev-only escape
hatches and hardcoded lists unless they are temporary migration scaffolding with
a clear deletion path.

Keep docs, tests, and tooling aligned in the same change set. A partial rewrite
that leaves behavior docs, feature tags, coverage, and mutation using different
identity systems will make the repo harder to reason about than it is today.
