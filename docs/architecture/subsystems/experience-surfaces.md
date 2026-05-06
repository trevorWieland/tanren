---
schema: tanren.subsystem_architecture.v0
subsystem: experience-surfaces
status: draft
owner_command: architect-system
updated_at: 2026-05-05
---

# Experience Surfaces Architecture

## Purpose

This document defines the proposed direction for making Tanren useful for
projects whose user experience is not primarily a web application.

Tanren should not generate "web UI" by default. It should generate and verify
surface-specific user experience from accepted product behavior. For Tanren
itself, the surfaces are currently `web`, `api`, `mcp`, `cli`, and `tui`. For an
adopting project, the surfaces might be a terminal application, game loop,
desktop GUI, mobile app, API, chat bot, hardware panel, library interface, or
other human or machine-facing surface.

The invariant stays the same: product behavior is the unit of meaning. What
changes is that interface IDs, proof harnesses, generated artifacts, and
experience standards become project-defined rather than hardcoded to Tanren's
own public surfaces.

## Current Gap

Tanren's current model is strong for Tanren itself but too specific for general
project adoption:

- behavior files use a closed interface vocabulary: `web`, `api`, `mcp`, `cli`,
  and `tui`;
- BDD scenario tags are validated against that same fixed set;
- web UX standards assume React, Storybook, Tailwind, Paraglide, Playwright, and
  browser accessibility tooling;
- phone reach is a global behavior-catalog concern rather than a property of the
  project surface model;
- Storybook is treated as the visual support mechanism, but games, terminals,
  libraries, embedded displays, and desktop apps need different proof forms;
- there is no first-class experience contract between behavior authoring and
  implementation.

The result is that Tanren can express "what the user can do", but it does not yet
have a portable way to express "where and how the user experiences it" across
non-web projects.

## Target Model

Each Tanren-managed project declares its own surfaces.

```yaml
surfaces:
  - id: terminal
    kind: human_text
    devices: [laptop]
    inputs: [keyboard]
    proof: [pty_transcript, golden_output]

  - id: gameplay
    kind: interactive_realtime
    devices: [desktop, handheld]
    inputs: [keyboard, controller]
    proof: [deterministic_replay, screenshot, frame_metrics]

  - id: api
    kind: machine_contract
    devices: [any]
    inputs: [http]
    proof: [contract_test, schema_test]
```

Surface IDs are project-local public experience contracts. They are not crate
names, rendering frameworks, or internal actors.

Tanren's own repository can continue to declare:

```yaml
surfaces:
  - id: web
    kind: responsive_gui
  - id: api
    kind: machine_contract
  - id: mcp
    kind: agent_tool_contract
  - id: cli
    kind: command_line
  - id: tui
    kind: terminal_ui
```

## Surface Record

A surface record should capture:

- stable surface ID;
- surface kind;
- target personas or clients;
- supported device classes;
- input methods;
- output modes;
- accessibility expectations;
- latency or performance expectations;
- supported localization or copy requirements;
- supported automation or test harnesses;
- proof artifact types;
- unsupported actions and explicit non-goals.

Candidate surface kinds:

- `responsive_gui`;
- `desktop_gui`;
- `mobile_gui`;
- `terminal_ui`;
- `command_line`;
- `human_text`;
- `interactive_realtime`;
- `turn_based_game`;
- `machine_contract`;
- `agent_tool_contract`;
- `chat_conversation`;
- `voice_conversation`;
- `embedded_display`;
- `library_api`.

The list should be extensible by project profiles. Tanren should ship useful
defaults, not a closed universal taxonomy.

## Experience Contracts

For each accepted behavior and surface pair, Tanren should generate or maintain
an experience contract.

An experience contract describes:

- behavior ID;
- surface ID;
- persona or client;
- entry point;
- primary task flow;
- required input path;
- success state;
- failure states;
- loading, empty, redacted, permission-denied, stale, and unavailable states;
- persistence or save-state expectations;
- timing or feedback expectations;
- accessibility expectations;
- localization and copy requirements;
- proof harness and evidence artifacts;
- human walk notes or review criteria.

The behavior remains the durable product contract. The experience contract is
the surface-specific interpretation that makes implementation and proof concrete.

## Method Chain Changes

The project method should become:

```text
plan-product
-> define-surfaces
-> identify-behaviors
-> design-experience
-> architect-system
-> assess-implementation
-> craft-roadmap
-> shape-spec / orchestrate / prove / walk
```

`define-surfaces` may be a separate command, or it may initially be part of
`architect-system`. It deserves a first-class phase because surface decisions
change behavior reach, proof harnesses, generated files, and roadmap sizing.

`design-experience` should bridge behavior and implementation. It should not
choose low-level component code before architecture is known, but it should
define surface-specific flows, states, and proof obligations early enough to
shape roadmap nodes correctly.

## Generated Artifacts

Portable Tanren projects should support an experience artifact root, likely:

```text
docs/experience/
  surfaces.yml
  flows.md
  screens.md
  interaction-models.md
  state-matrix.md
  proof-matrix.md
  accessibility.md
```

For projects with typed native Tanren storage, these should become projections
from typed experience records. During bootstrap, markdown and YAML projections
are acceptable if they have stable ownership and validators.

## Behavior File Changes

Behavior frontmatter should replace `interfaces` with a project-defined surface
field, or support `surfaces` as the successor field.

Current:

```yaml
interfaces: [web, api, mcp, cli, tui]
```

Proposed:

```yaml
surfaces: [terminal, gameplay, api]
```

The behavior catalog should validate surface IDs against the active project's
surface registry, not against Tanren's own hardcoded interface list.

For Tanren itself, a compatibility alias can map `interfaces` to `surfaces`
during migration.

## BDD And Proof Changes

The BDD tag allowlist must become project-configurable.

Current fixed tags:

```text
@web @api @mcp @cli @tui
```

Proposed:

```text
@<surface-id>
```

where `<surface-id>` is loaded from `docs/experience/surfaces.yml` or native
Tanren project configuration.

Proof validators should still enforce:

- one feature file per behavior where BDD is the chosen proof form;
- scenario tags cite valid surface IDs;
- positive and falsification coverage match the behavior's declared surfaces;
- proof adapters execute the real observable surface;
- skipped or ignored behavior scenarios are forbidden.

The proof adapter is what changes by surface kind.

## Proof Adapter Examples

### Web Or Browser GUI

Proof artifacts:

- Playwright BDD;
- screenshots across desktop and mobile viewports;
- accessibility scans;
- Storybook component states;
- visual regression snapshots.

Generated work:

- routes;
- components;
- i18n keys;
- stories;
- browser BDD steps;
- responsive screenshots.

### Command Line

Proof artifacts:

- process execution;
- stdout and stderr contracts;
- exit codes;
- structured JSON output tests;
- golden command transcripts.

Generated work:

- command grammar;
- help text;
- examples;
- error formatting;
- JSON schema for machine output;
- shell completion where relevant.

### Terminal UI

Proof artifacts:

- real PTY sessions;
- screen snapshots;
- keyboard navigation tests;
- resize tests;
- golden terminal transcripts.

Generated work:

- screen map;
- focus model;
- keyboard map;
- status bar;
- empty/error/permission states;
- operator recovery flows.

### Games

Proof artifacts:

- deterministic input replays;
- scene or level state assertions;
- screenshot or video captures;
- frame-time and latency metrics;
- save/load state checks;
- accessibility-option checks.

Generated work:

- mechanics contract;
- input map;
- scene/level entry points;
- feedback timing;
- progression and save-state rules;
- replay fixtures;
- frame budget tests.

Example:

```gherkin
@B-0201 @gameplay @positive
Scenario: Player retries a failed level
  Given the player failed level 3
  When the player presses retry
  Then level 3 restarts from its initial checkpoint
  And campaign progress is preserved
```

This should be backed by a deterministic replay or game-engine test harness, not
by a DOM test.

### Library Or SDK

Proof artifacts:

- public API contract tests;
- examples that compile or run;
- error taxonomy tests;
- compatibility tests;
- documentation snippets verified against real code.

Generated work:

- public API examples;
- usage guide;
- schema or type-level contract;
- negative cases;
- migration notes.

### Chat Or Agent Interface

Proof artifacts:

- conversation transcripts;
- tool-call traces;
- policy and refusal checks;
- state carryover checks;
- response-shape validation.

Generated work:

- prompt contract;
- allowed tools;
- conversation states;
- escalation behavior;
- audit attribution.

## Profile Changes

Tanren should ship project profiles that define surface defaults and proof
adapters.

Candidate profiles:

- `terminal-cli`;
- `terminal-tui`;
- `react-ts-pnpm`;
- `mobile-react-native`;
- `desktop-tauri`;
- `game-bevy`;
- `game-godot`;
- `game-unity`;
- `library-rust`;
- `library-typescript`;
- `api-service`;
- `chat-agent`.

Profiles should specify:

- supported surface kinds;
- generated artifact locations;
- proof harness commands;
- visual or transcript artifact rules;
- accessibility expectations;
- performance budgets;
- formatting and linting gates;
- example templates.

Tanren should detect existing project signals before applying a profile. A
non-React project should not inherit React-specific Storybook, Tailwind, or
Paraglide requirements unless it explicitly selects that profile.

## Roadmap And Spec Changes

Roadmap nodes should include surface and experience risk metadata.

Example:

```json
{
  "id": "R-0042",
  "completes_behaviors": ["B-0201"],
  "surface_scope": ["gameplay", "settings"],
  "experience_risk": "high",
  "expected_evidence": [
    {
      "kind": "deterministic_replay",
      "behavior_id": "B-0201",
      "surfaces": ["gameplay"],
      "witnesses": ["positive", "falsification"]
    },
    {
      "kind": "screenshot",
      "behavior_id": "B-0201",
      "surfaces": ["gameplay"]
    }
  ]
}
```

`experience_risk` should affect shaping:

- `low`: existing surface pattern, low interaction complexity;
- `medium`: new flow or multiple states, but known proof harness;
- `high`: new interaction model, real-time interaction, accessibility risk,
  critical user path, or proof harness uncertainty.

High-risk experience work should require stronger human walk evidence and richer
proof artifacts.

## Generation Workflow

When generating work for a behavior slice, Tanren should:

1. Read product intent, personas, concepts, accepted behavior, and surface
   registry.
2. Read existing project UI, interaction, command, or game patterns.
3. Create or update experience contracts for each behavior and surface pair.
4. Select the right profile and proof adapter.
5. Generate implementation scaffolding for the surface.
6. Generate proof scaffolding at the same time.
7. Generate review artifacts: screenshots, transcripts, replays, API examples,
   or other surface-native evidence.
8. Run full project gates.
9. Present the result in a human walk that references the accepted behavior and
   actual surface evidence.

Generated work should not stop at code. It should produce implementation,
tests, proof evidence, and reviewable experience artifacts together.

## Walk And Review Changes

Walks should review the actual experience artifact for the surface:

- web and mobile: screenshots, browser recording, accessibility scan, responsive
  checks;
- CLI: command transcript, help output, JSON output, exit codes;
- TUI: PTY transcript, screen snapshots, keyboard path;
- game: replay, screenshot or clip, frame metrics, save-state evidence;
- API/library: contract result, examples, error cases;
- chat: transcript, tool-call trace, policy outcome.

Walk acceptance should explicitly record:

- behavior reviewed;
- surface reviewed;
- evidence artifact reviewed;
- observed outcome;
- residual UX concerns;
- follow-up work or accepted deviations.

## Implementation Plan

## Bootstrap Implementation In This Repository

This repository now carries the first bootstrap layer of the model:

- `docs/experience/surfaces.yml` declares Tanren's current `web`, `api`, `mcp`,
  `cli`, and `tui` surfaces.
- `xtask check-bdd-tags` loads allowed scenario surface tags from that registry.
- Behavior `surfaces:` is supported, with existing `interfaces:` accepted as a
  migration alias.
- Roadmap `expected_evidence.surfaces` is supported, with
  `expected_evidence.interfaces` accepted as a migration alias.
- `scripts/roadmap_check.py` validates optional `surface_scope` and
  `experience_risk` metadata.
- `define-surfaces` and `design-experience` command sources define ownership
  for the new planning phases.
- Terminal CLI, terminal TUI, and generic game profiles seed non-web
  experience generation.

### Phase 1: Project Surface Registry

- Add a project surface registry projection.
- Allow Tanren itself to declare `web`, `api`, `mcp`, `cli`, and `tui`.
- Keep the existing `interfaces` field working for Tanren during migration.
- Update architecture docs so `interfaces.md` describes Tanren's own surfaces,
  while this document describes project-general surfaces.

### Phase 2: Validator Generalization

- Generalize `xtask check-bdd-tags` to load allowed surface tags from project
  configuration.
- Keep Tanren's current fixed set as the fallback only when
  `docs/experience/surfaces.yml` is absent.
- Keep validator messages in surface vocabulary while accepting `interfaces:`
  as a migration alias.

### Phase 3: Experience Contracts

- Add `docs/experience/` projections.
- Add an initial `design-experience` command or extend `architect-system` with a
  clearly owned section.
- Generate `state-matrix.md` and `proof-matrix.md` from behavior/surface pairs.

### Phase 4: Profiles And Proof Adapters

- Split current React assumptions into the `react-ts-pnpm` profile only.
- Add `terminal-cli` and `terminal-tui` profiles first because Tanren already has
  CLI and TUI surfaces.
- Add a `game-bevy` or generic `game` profile once the proof adapter model is
  stable.

### Phase 5: Roadmap And Spec Integration

- Add `surface_scope` and `experience_risk` to roadmap node guidance.
- Require shaped specs to identify experience contracts and proof adapters.
- Route UX or workflow gaps from demo/audit into findings or planning changes.

### Phase 6: Full Native Model

- Move surface registry, experience contracts, proof adapter config, and walk
  evidence into typed Tanren state.
- Render repo-local docs as projections from typed state.
- Detect drift in generated experience artifacts.

## Required Changes To Current Docs

When implementing this proposal, update:

- `docs/behaviors/index.md` to describe project-defined surfaces instead of a
  fixed interface list;
- `docs/architecture/subsystems/interfaces.md` to scope `web/api/mcp/cli/tui` as
  Tanren's own public surfaces;
- `docs/architecture/subsystems/behavior-proof.md` to describe surface-backed
  proof adapters;
- `tests/bdd/README.md` to reference surface tags loaded from project config;
- `profiles/react-ts-pnpm/**` to remove assumptions that belong only to web
  projects;
- `commands/project/*` to include `define-surfaces` and `design-experience`
  responsibilities;
- `docs/roadmap/dag.json` guidance and validators to understand surface scope
  and experience risk.

## Acceptance Criteria

This proposal is complete when:

- an adopting project can declare surfaces without using Tanren's own
  `web/api/mcp/cli/tui` IDs;
- behavior files can reference those project-defined surfaces;
- BDD or proof validators reject unknown surface IDs;
- proof adapters can be selected per surface kind;
- a non-web project can generate surface-specific experience contracts and proof
  obligations;
- React, Storybook, Tailwind, and Playwright are profile-specific, not universal
  UX assumptions;
- roadmap nodes can represent surface scope and experience risk;
- human walks review surface-native evidence rather than generic test status.

## Open Questions

- Should `define-surfaces` be a standalone command from the start, or should it
  be part of `architect-system` until native typed planning exists?
- Should the behavior frontmatter field be renamed from `interfaces` to
  `surfaces`, or should Tanren support both with a migration period?
- Should proof adapters be declared in profiles, in project config, or in native
  Tanren state first?
- What is the smallest game proof adapter that is useful without binding Tanren
  to one engine?
- Should visual, transcript, replay, and contract artifacts be stored in a shared
  evidence index, or should each proof adapter own its own projection format?
