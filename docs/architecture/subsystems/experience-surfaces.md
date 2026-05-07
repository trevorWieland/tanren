---
schema: tanren.subsystem_architecture.v0
subsystem: experience-surfaces
status: draft
owner_command: architect-system
updated_at: 2026-05-07
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
-> define-design-system
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

`define-design-system` follows `define-surfaces` and precedes
`design-experience`. It records the shared tokens, vocabulary, pattern IDs,
accessibility expectations, and surface adapters that generated work must
follow so agents do not invent local interaction rules while implementing a
behavior slice.

`design-experience` should bridge behavior and implementation. It should not
choose low-level component code before architecture is known, but it should
define surface-specific flows, states, and proof obligations early enough to
shape roadmap nodes correctly.

## Generated Artifacts

Portable Tanren projects should support an experience artifact root, likely:

```text
docs/experience/
  surfaces.yml
  design-system/
    README.md
    principles.md
    tokens.yml
    vocabulary.yml
    patterns.yml
    accessibility.md
    surface-adapters/
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

## Behavior Files

Behavior frontmatter uses a project-defined `surfaces:` field validated
against the active surface registry:

```yaml
surfaces: [terminal, gameplay, api]
```

The behavior catalog rejects unknown surface IDs at validation time. There is
no `interfaces:` compatibility field; Tanren's own catalog uses `surfaces:`
just like adopting projects.

## BDD And Proof

The BDD tag allowlist is project-configurable. Scenarios use:

```text
@<surface-id>
```

where `<surface-id>` is loaded from `docs/experience/surfaces.yml`.

Proof validators enforce:

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

Roadmap nodes should include surface, design-pattern, and experience-risk
metadata.

Example:

```json
{
  "id": "R-0042",
  "completes_behaviors": ["B-0201"],
  "surface_scope": ["gameplay", "settings"],
  "experience_risk": "high",
  "design_pattern_refs": ["gameplay.retry.preserve-progress"],
  "expected_evidence": [
    {
      "kind": "deterministic_replay",
      "behavior_id": "B-0201",
      "surfaces": ["gameplay"],
      "design_pattern_refs": ["gameplay.retry.preserve-progress"],
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
2. Read the design-system records and selected pattern IDs.
3. Read existing project UI, interaction, command, or game patterns.
4. Create or update experience contracts for each behavior and surface pair.
5. Select the right profile and proof adapter.
6. Generate implementation scaffolding for the surface.
7. Generate proof scaffolding at the same time.
8. Generate review artifacts: screenshots, transcripts, replays, API examples,
   or other surface-native evidence.
9. Run full project gates.
10. Present the result in a human walk that references the accepted behavior,
    surface, design pattern IDs, and actual surface evidence.

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

## Bootstrap State In This Repository

The first bootstrap layer of the model is in place:

- `docs/experience/surfaces.yml` declares Tanren's `web`, `api`, `mcp`, `cli`,
  and `tui` surfaces in the new schema.
- Tanren's behavior catalog (288 accepted behaviors) and roadmap DAG (288
  evidence entries) use `surfaces:` exclusively; there is no `interfaces:`
  compatibility field.
- `xtask check-bdd-tags` loads allowed scenario surface tags from the
  registry. `scripts/roadmap_check.py` validates `surface_scope` and
  `experience_risk` against the same registry and validates
  `design_pattern_refs` against the design-system pattern registry.
- `define-surfaces`, `define-design-system`, and `design-experience` command
  sources define ownership for the new planning phases. `B-0289`–`B-0294` are
  the user-facing capabilities Tanren itself must implement to make these
  phases first-class rather than method-only conventions; `R-0290`–`R-0295`
  are the completing roadmap nodes.
- Terminal CLI, terminal TUI, and generic game profiles seed non-web
  experience generation.

## Remaining Roadmap

The follow-on work tracked outside this PR:

- Move the surface registry, experience contracts, proof adapter config, and
  walk evidence into typed Tanren state, with repo-local docs becoming
  projections from that state.
- Move design-system tokens, vocabulary, pattern IDs, surface adapters, and
  design drift decisions into typed Tanren state.
- Add additional non-web profiles (`game-bevy`, `mobile-react-native`,
  `desktop-tauri`, `chat-agent`, `library-rust`, …) as adopting projects need
  them.
- Tighten the React/Storybook/Tailwind/Paraglide assumptions that currently
  live workspace-wide so they only apply when the `react-ts-pnpm` profile is
  selected.
- Detect drift in generated experience artifacts during walks.

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
- roadmap nodes can reference registered design pattern IDs;
- human walks review surface-native evidence rather than generic test status.

## Open Questions

- Should proof adapters be declared in profiles, in project config, or in native
  Tanren state first?
- What is the smallest game proof adapter that is useful without binding Tanren
  to one engine?
- Should visual, transcript, replay, and contract artifacts be stored in a shared
  evidence index, or should each proof adapter own its own projection format?
