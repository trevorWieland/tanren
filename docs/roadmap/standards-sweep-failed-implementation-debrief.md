# Standards Sweep Failed Implementation Debrief

Status: excision handoff  
Date: 2026-04-27  
Failed spec: `e048284c-3508-4d9d-b972-59c20e084abb`

## Executive Summary

The current standards sweep implementation should be treated as a failed
direction and systematically removed before any fresh attempt is shaped.
It does not implement the intended product model.

The intended model was a project-level standards sweep:

- not tied to any active spec;
- runnable as a simple project command, ideally with no required arguments;
- responsible for discovering standards, inspecting the repository, and
  producing fresh findings;
- broad by default, not limited to touched files or an active task/spec diff;
- standards-agnostic, with interpretation coming from standards content and a
  generic review mechanism, not hard-coded evaluators.

The implemented model is materially different:

- sweep state is recorded inside spec-scoped phase-event streams;
- APIs and services require an active `spec_id` / `spec_folder`;
- the proof script requires externally supplied `review-results.json`;
- "sweep" is actually typed ingestion of preexisting findings;
- operator inputs include touched files, domains, project language, filters,
  artifact roots, log roots, and spec runtime fields;
- tests and demo artifacts prove the wrong behavior.

Do not polish this implementation. Excise it, then take stock before deciding
whether to shape a fresh standards-sweep spec.

## Intended Contract

The command surface should start from this mental model:

```sh
scripts/orchestration/sweep-standards.sh
```

That command should:

1. Read repo configuration.
2. Load the standards catalog.
3. Walk the relevant repository contents by default.
4. Use a generic subjective/static review mechanism to evaluate the codebase
   against the loaded standards.
5. Generate fresh findings.
6. Persist and project useful human-readable sweep output.
7. Optionally feed synthesis/triage after findings exist.

The script should not require existing findings as input. If findings already
exist, the sweep has not done the core work it is named for.

## Where It Went Wrong

### 1. Sweep State Became Spec-Scoped

The implementation stores sweep events as methodology events under an active
spec runtime. That is the opposite of the project-governance model.

Concrete examples:

- `crates/tanren-domain/src/methodology/sweep_events.rs`
  - every sweep event carries `spec_id: SpecId`;
  - `StandardsSweepPlanned`, `StandardsSweepAttemptStarted`,
    `StandardsSweepStandardResultRecorded`, `StandardsSweepCompleted`,
    `StandardsSweepSynthesized`, and
    `StandardsSweepTriageDecisionRecorded` are all spec-owned.
- `crates/tanren-domain/src/methodology/events.rs`
  - sweep events are methodology events whose entity/spec routing points back
    to `SpecId`.
- `crates/tanren-app-services/src/methodology/service_sweep.rs`
  - `runtime_spec_id()` is required for plan, start, result, complete,
    synthesis recording, triage decision recording, and status reads.
  - error remediation explicitly says to pass `--spec-id` and
    `--spec-folder`.
- `crates/tanren-app-services/src/methodology/service_sweep_runner.rs`
  - `require_runtime_spec_id()` blocks autonomous sweep runs unless a spec
    runtime exists.
- `crates/tanren-app-services/src/methodology/service_sweep_synthesis.rs`
  - `require_synthesis_runtime_spec_id()` makes synthesis spec-runtime-bound.
- `crates/tanren-app-services/src/methodology/service_sweep_triage.rs`
  - `require_triage_runtime_spec_id()` makes triage spec-runtime-bound;
  - issue creation uses `origin_spec_id: spec_id`.

This makes standards sweep a spec artifact instead of project governance.

### 2. The Script Does Not Sweep

`scripts/orchestration/sweep-standards.sh` is described as an on-demand proof
driver, but functionally it is a recorder for preexisting review results.

Concrete failures:

- requires `--review-results`;
- exits immediately when `--review-results` is absent;
- reads the supplied JSON into `review_results`;
- passes those results directly to `tanren-cli methodology ... sweep run`;
- never performs a repository-wide review itself;
- never generates fresh findings from standards.

That is backwards. A command named `sweep-standards` must be the thing that
discovers findings.

### 3. The API Encodes Preexisting Findings

The core contract bakes in the same wrong shape.

`crates/tanren-contract/src/methodology/sweep.rs` defines:

- `RunStandardsSweepParams.review_results`;
- `RunStandardsSweepReviewResult`;
- `RunStandardsSweepParams.touched_files`;
- `RunStandardsSweepParams.project_language`;
- `RunStandardsSweepParams.domains`;
- `RunStandardsSweepParams.tags`;
- `RunStandardsSweepParams.category`;
- `RunStandardsSweepParams.filter`.

`crates/tanren-app-services/src/methodology/service_sweep_runner.rs` then:

- selects standards based on those inputs;
- requires one review result for every selected standard;
- rejects missing review results;
- records those externally supplied results as terminal attempt outcomes.

This makes the implementation a typed evidence ingestion API, not a sweep.

### 4. Scope Narrowing Replaced Whole-Repo Review

The implementation repeatedly narrows scope with touched files, path globs,
domain filters, project language, tags, categories, and selected standards.

Some of those concepts may make sense elsewhere in Tanren standards selection,
especially adherence or targeted standard discovery. They do not match the
default sweep command the product needed here. A sweep should start broad and
derive applicability from the standards catalog and repository content, not
from an operator-supplied active-file list.

Key places:

- `RunStandardsSweepParams.touched_files`;
- `RunStandardsSweepParams.project_language`;
- `RunStandardsSweepParams.domains`;
- `RunStandardsSweepParams.tags`;
- `RunStandardsSweepParams.category`;
- `StandardsSweepFilter.path_globs`;
- `service_sweep_runner.rs::select_sweep_standards`;
- `scripts/orchestration/sweep-standards.sh` CLI flags:
  `--standard`, `--touched-file`, `--domain`, `--project-language`.

### 5. The Command Markdown Teaches the Wrong Workflow

The command docs are internally conflicted. They call the command
project-wide, but then tell agents to record supplied review results rather
than generate findings.

Files:

- `commands/project/sweep-standards.md`;
- generated copies:
  - `.codex/skills/sweep-standards/SKILL.md`;
  - `.claude/commands/sweep-standards.md`;
  - `.opencode/commands/sweep-standards.md`.

Wrong ideas encoded there:

- optional filters for paths, languages, domains, tags;
- "provide the selected standard and repository context to the autonomous
  subjective static review workflow, then record the supplied result";
- sweep artifacts under `tanren/standards-sweeps` as proof of completion even
  though the sweep did not itself inspect the repo.

### 6. The Proof and Demo Artifacts Prove the Wrong Thing

The deleted active spec claimed success through a demo step that ran the proof
driver with hand-authored review results. That is not acceptance evidence for
a standards sweep.

Removed active spec folder:

- `tanren/specs/e048284c-3508-4d9d-b972-59c20e084abb-standards-sweep`

Generated runtime artifacts still exist outside that folder and should be
cleaned during excision:

- `tanren/standards-sweeps/proof-spec`;
- `tanren/standards-sweeps/proof`;
- `tanren/standards-sweeps/proof-logs`.

These were local pollution from the failed proof path and should not be
retained or ignored after excision.

### 7. BDD Coverage Locks In the Wrong Behavior

The behavior tests currently validate evidence ingestion and proof-driver
behavior, not a real sweep.

Files:

- `tests/bdd/features/standards-sweep/contracts.feature`;
- `crates/tanren-bdd/src/steps/sweep.rs`;
- `crates/tanren-bdd/src/steps/sweep_command.rs`;
- `crates/tanren-bdd/src/steps/sweep_command_agnostic.rs`;
- `crates/tanren-bdd/src/steps/sweep_synthesis.rs`;
- `crates/tanren-bdd/src/steps/sweep_triage.rs`;
- `crates/tanren-bdd/src/steps/sweep_artifact_helpers.rs`;
- `crates/tanren-bdd/src/steps/mod.rs`.

Problematic scenarios include:

- "Sweep command records constrained subjective review results end to end";
- "Sweep command accepts arbitrary qualitative standards through review
  results";
- "Sweep command rejects missing subjective review output";
- "Sweep proof driver reaches the triage checkpoint".

These scenarios prove that externally supplied findings are recorded. They do
not prove that a sweep generates findings.

### 8. Synthesis and Triage Sit on the Wrong Substrate

The synthesis and triage ideas may be useful later, but this implementation is
coupled to the wrong event model.

Files:

- `crates/tanren-app-services/src/methodology/service_sweep_synthesis.rs`;
- `crates/tanren-app-services/src/methodology/service_sweep_triage.rs`;
- `commands/project/synthesize-sweep.md`;
- `commands/project/triage-sweep.md`;
- generated `.codex`, `.claude`, `.opencode` command copies.

Specific issues:

- synthesis reads spec-scoped event history;
- triage requires a spec runtime;
- triage-created issues inherit `origin_spec_id`;
- the only available input is already-recorded external findings.

If synthesis/triage are rebuilt later, they should consume project-sweep
findings from a project-level run, not a spec-scoped methodology event stream.

## Inventory For Excision

### Source Modules To Remove Or Rework

Domain:

- `crates/tanren-domain/src/methodology/sweep.rs`;
- `crates/tanren-domain/src/methodology/sweep_events.rs`;
- sweep exports in `crates/tanren-domain/src/methodology/mod.rs`;
- sweep variants in `crates/tanren-domain/src/methodology/events.rs`;
- sweep IDs in `crates/tanren-domain/src/ids.rs`;
- sweep phases in `crates/tanren-domain/src/methodology/phase_id.rs`;
- sweep capabilities in `crates/tanren-domain/src/methodology/capability.rs`;
- sweep tools in `crates/tanren-domain/src/methodology/tool_catalog.rs`;
- sweep event/tool attribution in
  `crates/tanren-domain/src/methodology/event_tool.rs`.

Contract:

- `crates/tanren-contract/src/methodology/sweep.rs`;
- sweep exports in `crates/tanren-contract/src/methodology/mod.rs`.

App services:

- `crates/tanren-app-services/src/methodology/service_sweep.rs`;
- `crates/tanren-app-services/src/methodology/service_sweep_runner.rs`;
- `crates/tanren-app-services/src/methodology/service_sweep_projection.rs`;
- `crates/tanren-app-services/src/methodology/service_sweep_artifacts.rs`;
- `crates/tanren-app-services/src/methodology/service_sweep_synthesis.rs`;
- `crates/tanren-app-services/src/methodology/service_sweep_triage.rs`;
- module wiring in `crates/tanren-app-services/src/methodology/mod.rs`;
- installer command bindings in
  `crates/tanren-app-services/src/methodology/installer_binding.rs`;
- phase capability output in
  `crates/tanren-app-services/src/methodology/service_phase.rs`.

CLI/MCP:

- `bin/tanren-cli/src/commands/methodology/sweep.rs`;
- sweep subcommand wiring in
  `bin/tanren-cli/src/commands/methodology/mod.rs`;
- sweep MCP tool registry entries in `bin/tanren-mcp/src/tool_registry.rs`.

Script:

- `scripts/orchestration/sweep-standards.sh`.

### Command Files To Remove Or Rework

Source command markdown:

- `commands/project/sweep-standards.md`;
- `commands/project/synthesize-sweep.md`;
- `commands/project/triage-sweep.md`.

Generated command artifacts:

- `.codex/skills/sweep-standards/SKILL.md`;
- `.codex/skills/synthesize-sweep/SKILL.md`;
- `.codex/skills/triage-sweep/SKILL.md`;
- `.claude/commands/sweep-standards.md`;
- `.claude/commands/synthesize-sweep.md`;
- `.claude/commands/triage-sweep.md`;
- `.opencode/commands/sweep-standards.md`;
- `.opencode/commands/synthesize-sweep.md`;
- `.opencode/commands/triage-sweep.md`.

Also remove references from command indexes if present:

- `commands/README.md`;
- generated command indexes, if the installer owns them.

### Documentation To Remove Or Rewrite

- `docs/architecture/standards-sweep.md`;
- `docs/behaviors/B-0072-record-standards-sweep-evidence.md`;
- references in `docs/behaviors/README.md`;
- references in `docs/architecture/agent-tool-surface.md`;
- references in `docs/architecture/phase-taxonomy.md`;
- references in `docs/architecture/overview.md`;
- references in `docs/architecture/adherence.md`;
- references in `docs/architecture/orchestration-flow.md`;
- references in `docs/methodology/system.md`;
- references in `docs/README.md`.

### Tests To Remove Or Rewrite

- `tests/bdd/features/standards-sweep/contracts.feature`;
- `crates/tanren-bdd/src/steps/sweep.rs`;
- `crates/tanren-bdd/src/steps/sweep_command.rs`;
- `crates/tanren-bdd/src/steps/sweep_command_agnostic.rs`;
- `crates/tanren-bdd/src/steps/sweep_synthesis.rs`;
- `crates/tanren-bdd/src/steps/sweep_triage.rs`;
- `crates/tanren-bdd/src/steps/sweep_artifact_helpers.rs`;
- module wiring in `crates/tanren-bdd/src/steps/mod.rs`;
- any `BehaviorWorld` fields dedicated only to sweep runs.

### Local Generated State To Clean

These are not source truth and should be removed locally during excision:

- `tanren/standards-sweeps/proof-spec`;
- `tanren/standards-sweeps/proof`;
- `tanren/standards-sweeps/proof-logs`;
- any remaining `tanren/standards-sweeps/**` output.

The active spec folder has already been deleted:

- `tanren/specs/e048284c-3508-4d9d-b972-59c20e084abb-standards-sweep`.

The local `tanren.db` also contains sweep-related payloads in the generic
methodology event stream. A source-code excision will not remove that local
database history. Decide separately whether to reset the local database,
append compensating events, or leave it as local scratch state.

## What Not To Remove By Accident

Do not confuse the failed sweep implementation with the standards runtime
itself. These concepts may still be valid:

- standards catalog files under `tanren/standards`;
- standards discovery/index/injection commands:
  - `commands/project/discover-standards.md`;
  - `commands/project/index-standards.md`;
  - `commands/project/inject-standards.md`;
- runtime standards behavior from `docs/behaviors/B-0071-load-runtime-standards-root.md`;
- standard domain/contract models:
  - `crates/tanren-domain/src/methodology/standard.rs`;
  - `crates/tanren-contract/src/methodology/standard.rs`;
- standard listing services:
  - `crates/tanren-app-services/src/methodology/standards.rs`;
  - `crates/tanren-app-services/src/methodology/service_standards.rs`;
  - `bin/tanren-cli/src/commands/methodology/standard.rs`.

These may need cleanup if they reference sweep-specific filters or command
names, but they are not automatically part of the failed implementation.

## Suggested Excision Order

1. Delete the project command source files for sweep/synthesize/triage and run
   the installer to remove generated command artifacts.
2. Remove the shell proof driver.
3. Remove CLI and MCP sweep tool registration.
4. Remove contract sweep params and exports.
5. Remove domain sweep models, events, IDs, phases, capabilities, and tool
   catalog entries.
6. Remove app-service sweep modules and projection hooks.
7. Remove BDD features and step modules.
8. Remove architecture/behavior docs that describe the failed sweep model.
9. Remove ignored local generated `tanren/standards-sweeps/**` output.
10. Run full repo gates only:
    - `just check`
    - `just ci`
    - `tanren-cli install --config tanren.yml --dry-run`

Do not replace this with another implementation in the same pass. First get
the codebase back to a coherent state without the failed sweep surface.

## Criteria For Any Future Fresh Shape

A future shape-spec should make these acceptance constraints explicit:

- `sweep-standards` is project-scoped, not spec-scoped.
- The default operator command requires no spec ID, no spec folder, no touched
  files, no domain, no project language, and no preexisting findings.
- The sweep command generates findings from repository inspection.
- Any optional narrowing is secondary and must not be required for the primary
  command path.
- Production code must not contain standard-specific evaluators.
- The generic review mechanism must be able to handle arbitrary qualitative
  standards.
- Generated findings must preserve enough evidence for synthesis/triage, but
  the existence of synthesis/triage must not be used as proof that sweeping
  itself works.
