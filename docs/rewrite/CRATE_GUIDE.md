# Tanren Clean-Room Rewrite: Crate Guide

## Overview

This guide defines the target Rust workspace shape for the tanren rewrite.
It mirrors the forgeclaw style: strict crate boundaries, explicit dependency
rules, and `just`-driven workflow orchestration.

## Workspace Topology (Proposed)

```text
bin/
  tanrend             # daemon/control-plane runtime
  tanren-api          # HTTP API service
  tanren-cli          # CLI entrypoint
  tanren-mcp          # MCP server entrypoint
  tanren-tui          # optional terminal UI

crates/
  domain              # canonical entities, commands, events, state machine enums
  contract            # schema/versioning + interface contract generation
  policy              # authz, quotas, budgets, placement policy decisions
  store               # event log, projections, migrations, repository APIs
  planner             # task graph planning + replanning logic
  scheduler           # dispatch graph scheduling and lane/capability routing
  runtime             # runtime traits (harness + environment) and shared types
  runtime-local       # local worktree execution runtime
  runtime-docker      # local docker + DooD execution runtime
  runtime-remote      # remote VM/cloud runtime
  harness-claude      # Claude Code harness adapter
  harness-codex       # Codex harness adapter
  harness-opencode    # OpenCode harness adapter
  orchestrator        # control-plane orchestration engine (planner + policy + scheduler coordination)
  observability       # tracing, metrics, correlation helpers
  app-services        # shared application service layer used by API/CLI/MCP/TUI
```

## Dependency Graph

```text
bin/*
 ├── app-services
 │    ├── orchestrator
 │    │    ├── planner
 │    │    ├── scheduler
 │    │    ├── policy
 │    │    ├── store
 │    │    ├── runtime
 │    │    └── domain
 │    ├── contract
 │    └── observability
 └── harness-* + runtime-* (for wiring/composition as needed)

domain
  └── (no internal deps)

contract
  └── domain

policy
  └── domain

store
  └── domain

planner
  └── domain

scheduler
  ├── domain
  └── policy

runtime
  └── domain

runtime-local / runtime-docker / runtime-remote
  ├── runtime
  ├── domain
  └── policy

harness-*
  ├── runtime
  ├── domain
  └── policy

orchestrator
  ├── planner
  ├── scheduler
  ├── policy
  ├── store
  ├── runtime
  └── domain

observability
  └── domain

app-services
  ├── orchestrator
  ├── contract
  ├── policy
  ├── store
  └── observability
```

No circular dependencies are allowed.

## Crate Responsibilities

### `domain`

Owns canonical semantics:

- domain IDs/newtypes
- lifecycle states
- commands and events
- typed error taxonomy
- invariant helpers

No external runtime/storage concerns.

### `contract`

Owns external contract representation and versioning:

- API schema mapping
- MCP tool schema mapping
- CLI command schema mapping
- compatibility/version policy

Must not own business logic.

### `policy`

Owns authorization and governance decisions:

- identity scopes
- budget/quota limits
- placement approvals/denials
- decision reason model for audit

### `store`

Owns persistence:

- event append APIs
- projection read/write APIs
- migration lifecycle
- transactional guards for race-safe operations

### `planner`

Owns decomposition intelligence:

- issue/task graph planning
- dependency graph updates
- replanning triggers and outputs

### `scheduler`

Owns execution ordering:

- lane/capability-aware queueing
- backpressure
- scheduling decisions based on policy + capacity

### `runtime`

Owns runtime contracts:

- harness trait
- environment lease trait
- normalized run result/event models

### `runtime-*`

Own concrete environment runtime implementations:

- local worktree
- docker + DooD
- remote VM/cloud

### `harness-*`

Own concrete harness integrations:

- command/prompt preparation
- execution + stream handling
- telemetry and error normalization

### `orchestrator`

Owns control-plane orchestration loop:

- command intake path
- planner/scheduler/policy/store/runtime coordination
- state transition orchestration

### `observability`

Owns shared telemetry primitives:

- tracing context propagation
- metrics registry setup
- audit/event correlation helpers

### `app-services`

Owns interface-facing application service layer:

- stable use-case APIs for API/CLI/MCP/TUI
- input mapping and output shaping
- workflow-context and methodology-boundary resolution for interface-facing operations
- no direct transport assumptions

## Linking Rules

1. Core rule: `domain` never imports from any other workspace crate.
2. Transport rule: interface binaries only depend on `app-services` + `contract`.
3. Storage rule: only `store` owns SQL/query details.
4. Runtime rule: environment and harness crates never own policy decisions.
5. Policy rule: policy returns typed decisions, never transport-layer errors.
6. Contract rule: contract crate is serialization/schema only, no orchestration logic.
7. Methodology rule: command rendering and workflow-context resolution are
   control-plane/application concerns, not prompt-local logic.
8. Observability rule: no crate emits unstructured logs without correlation context.

## Workspace and Version Management

Use centralized dependency pinning:

- `[workspace.dependencies]` for shared crates
- `[workspace.lints.*]` for shared lint policy
- per-crate deviations only with explicit rationale

Set `resolver = "2"` and use edition `2024`.

## `just` Orchestration Model

Adopt `just` as the single developer task runner for the Rust workspace.

### Why `just`

- ergonomic command recipes
- clear discoverability (`just --list`)
- simpler multi-step scripts than Make
- consistent local/CI invocation

### Baseline Recipes

- `bootstrap` - install toolchain/components/dev tools
- `build` - workspace build
- `check` - type/lint checks
- `fmt` / `fmt-fix` - rustfmt + toml formatting checks/fixes
- `lint` - clippy strict
- `test` - nextest workspace tests
- `coverage` - llvm-cov nextest coverage
- `deny` - dependency policy checks
- `doc` - docs build with warnings denied
- `machete` - unused dependency detection
- `ci` - full quality gate pipeline

### Quality Gates

Match forgeclaw-style strictness:

- deny warnings in clippy and rustdoc
- deny inline lint suppression unless explicitly approved policy exists
- file size/complexity thresholds where useful
- dependency/source/license policy enforced in CI

## CI Alignment

CI should call `just` recipes directly so local and CI flows are identical.

Minimal CI stages:

1. `fmt`
2. `lint`
3. `test`
4. `deny`
5. `doc`
6. `machete`
7. optional `coverage`

## Migration Strategy Note

This crate map is for the rewrite branch stream. It should not be mixed into the
current Python service tree until cutover planning explicitly reaches migration
phase gates.
