---
schema_version: v1
kind: spec
spec_id: 00000000-0000-0000-0000-000000000c01
title: 'Hard Cutover: Behavior-First Rust Testing for Phase 0 Proof'
problem_statement: The Rust refactor test suite is not compliant with the new behavior-first testing framework standards. Current proof relies on mixed nextest/unit/integration and proof scripts without standards-required BDD-first scenario execution, mutation-strength gating, and behavior-proxy coverage interpretation. We need a hard cutover that resets Rust test assets and rebuilds them systematically against Phase 0 behaviors while preserving per-task demonstrability via just check and final closure via just ci.
motivations:
- Enforce standards compliance for Rust testing.
- Eliminate dual-framework ambiguity and drift.
- Improve behavior proof quality with positive and falsification witnesses.
- Add mutation-strength enforcement to detect weak tests.
- Treat coverage as behavior-gap signal rather than vanity metric.
- Trial Tanren orchestration in self-host mode and assess readiness.
expectations:
- Every in-scope Phase 0 behavior maps to stable behavior IDs and executable scenarios.
- Each implementation task ends with just check green and demonstrable stage output.
- just ci remains the final full gate for spec completion.
- Mutation and coverage gates are first-class and blocking at final enforcement.
- Rust test cutover leaves no unmanaged legacy behavior-test path.
- Meta-evaluation artifact captures orchestration friction and readiness findings.
planned_behaviors:
- PB-01 Standards baseline includes Rust testing standards and relevance context for this spec.
- PB-02 Phase 0 behavior inventory maps Feature 1.1 through 8.1 to stable behavior IDs and scenarios.
- PB-03 Scenario files (.feature) become the authoritative behavior proof path.
- PB-04 Hard cutover deletes legacy Rust test suite and rebuilds incrementally by behavior waves.
- PB-05 Nextest remains required for support/test-binary discipline alongside BDD scenarios.
- PB-06 Mutation gate runs in CI/local; survivors require behavior-ID-linked triage.
- PB-07 Coverage gate reports missing behavior vs dead/support code classification.
- PB-08 Skip suppression is enforced for Rust behavior suites.
- PB-09 justfile, pre-commit, CI workflows, and check-ci-parity remain synchronized.
- PB-10 Phase 0 proof docs/scripts/evidence index reflect new testing flow.
- PB-11 Orchestration trial outputs a meta-readiness report with actionable recommendations.
implementation_plan:
- Sync/inject Rust testing standards into active standards set.
- Define behavior inventory and traceability schema for Phase 0 behaviors.
- Introduce gate scaffolding early so each task can prove function and keep just check green.
- Scaffold cucumber-rs harness and execute initial vertical slices.
- Execute hard-cut milestone deleting legacy Rust behavior tests.
- Rebuild behavior coverage in waves until all in-scope behaviors are proven.
- Wire mutation and coverage gates with behavior-linked triage/classification artifacts.
- Finalize strict enforcement in justfile/lefthook/CI/parity once suites are stable.
- Refresh Phase 0 proof scripts/docs/evidence mapping for the new framework.
- Produce meta-readiness report on Tanren command orchestration trial.
non_negotiables:
- Scope is Rust test replacement only; Python legacy deletion is out of scope for this spec.
- No long-lived dual old/new Rust behavior-test stacks.
- Each task must end with just check green plus stage-specific demonstrable output.
- just ci is the final spec gate before closure.
- No skipped behavior scenarios or ignore-based suppression in the cutover suite.
- Every behavior-changing claim must map to behavior IDs and scenarios.
- Mutation regressions fail at final enforced gate.
- Coverage interpretation remains behavior-first with explicit classification.
- Gate parity across local hooks and CI must be preserved.
- Orchestrator-owned artifacts remain tool-driven only.
- Meta-readiness output is mandatory for this trial run.
acceptance_criteria:
- id: ac-01
  description: Active standards include Rust testing rules for BDD, mutation, coverage, no-skip, and traceability.
  measurable: tanren/standards contains testing standards and relevance filters include this spec scope.
- id: ac-02
  description: Behavior inventory maps all selected Phase 0 behaviors to stable IDs and scenario files.
  measurable: Traceability artifact lists Feature 1.1-8.1 mappings with scenario references.
- id: ac-03
  description: Scenario suite provides positive and falsification witnesses for each mapped behavior.
  measurable: Each mapped behavior has at least one passing positive and one passing falsification scenario.
- id: ac-04
  description: Hard cutover removes legacy Rust behavior-test paths and documents the post-cutover allowed test forms.
  measurable: Legacy Rust tests are deleted from agreed paths and migration policy doc is updated.
- id: ac-05
  description: justfile exposes staged behavior/mutation/coverage commands and final strict gates.
  measurable: just recipes run successfully at intended stage and are referenced by CI parity checks.
- id: ac-06
  description: Each task demonstrates functional progress and ends with just check green.
  measurable: Task evidence includes stage command output plus successful just check run.
- id: ac-07
  description: Mutation gate is blocking at final enforcement with survivor triage tied to behavior IDs.
  measurable: CI/job failure occurs on regression and survivor records reference BEH-* IDs.
- id: ac-08
  description: Coverage output includes classification of uncovered paths.
  measurable: Coverage report includes missing-behavior vs dead/support code categories.
- id: ac-09
  description: lefthook, GitHub workflows, and check-ci-parity validate the updated test model.
  measurable: Local hooks and workflow commands pass parity guard with no drift.
- id: ac-10
  description: Phase 0 proof scripts/docs are updated and reproducible with new test framework.
  measurable: Runbook and evidence index commands regenerate expected proof artifacts.
- id: ac-11
  description: Meta-evaluation records orchestration trial outcomes and recommendations.
  measurable: A committed report captures strengths, friction points, and go/no-go guidance.
demo_environment:
  connections:
  - name: workspace-fs
    kind: fs
    probe: test -d . && test -f justfile
  - name: docker-daemon
    kind: fs
    probe: docker info
  - name: ci-surface
    kind: http
    probe: GitHub Actions workflow execution (remote)
dependencies: {}
base_branch: rewrite/tanren-2-foundation
created_at: 2026-04-22T14:14:08.137432Z
---
# Spec

## Problem Statement
The Rust refactor test suite is not compliant with the new behavior-first testing framework standards. Current proof relies on mixed nextest/unit/integration and proof scripts without standards-required BDD-first scenario execution, mutation-strength gating, and behavior-proxy coverage interpretation. We need a hard cutover that resets Rust test assets and rebuilds them systematically against Phase 0 behaviors while preserving per-task demonstrability via just check and final closure via just ci.

## Motivations
- Enforce standards compliance for Rust testing.
- Eliminate dual-framework ambiguity and drift.
- Improve behavior proof quality with positive and falsification witnesses.
- Add mutation-strength enforcement to detect weak tests.
- Treat coverage as behavior-gap signal rather than vanity metric.
- Trial Tanren orchestration in self-host mode and assess readiness.

## Expectations
- Every in-scope Phase 0 behavior maps to stable behavior IDs and executable scenarios.
- Each implementation task ends with just check green and demonstrable stage output.
- just ci remains the final full gate for spec completion.
- Mutation and coverage gates are first-class and blocking at final enforcement.
- Rust test cutover leaves no unmanaged legacy behavior-test path.
- Meta-evaluation artifact captures orchestration friction and readiness findings.

## Planned Behaviors
- PB-01 Standards baseline includes Rust testing standards and relevance context for this spec.
- PB-02 Phase 0 behavior inventory maps Feature 1.1 through 8.1 to stable behavior IDs and scenarios.
- PB-03 Scenario files (.feature) become the authoritative behavior proof path.
- PB-04 Hard cutover deletes legacy Rust test suite and rebuilds incrementally by behavior waves.
- PB-05 Nextest remains required for support/test-binary discipline alongside BDD scenarios.
- PB-06 Mutation gate runs in CI/local; survivors require behavior-ID-linked triage.
- PB-07 Coverage gate reports missing behavior vs dead/support code classification.
- PB-08 Skip suppression is enforced for Rust behavior suites.
- PB-09 justfile, pre-commit, CI workflows, and check-ci-parity remain synchronized.
- PB-10 Phase 0 proof docs/scripts/evidence index reflect new testing flow.
- PB-11 Orchestration trial outputs a meta-readiness report with actionable recommendations.

## Ordered Implementation Plan
1. Sync/inject Rust testing standards into active standards set.
2. Define behavior inventory and traceability schema for Phase 0 behaviors.
3. Introduce gate scaffolding early so each task can prove function and keep just check green.
4. Scaffold cucumber-rs harness and execute initial vertical slices.
5. Execute hard-cut milestone deleting legacy Rust behavior tests.
6. Rebuild behavior coverage in waves until all in-scope behaviors are proven.
7. Wire mutation and coverage gates with behavior-linked triage/classification artifacts.
8. Finalize strict enforcement in justfile/lefthook/CI/parity once suites are stable.
9. Refresh Phase 0 proof scripts/docs/evidence mapping for the new framework.
10. Produce meta-readiness report on Tanren command orchestration trial.
