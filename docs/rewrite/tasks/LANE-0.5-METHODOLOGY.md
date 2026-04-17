# Lane 0.5 — Methodology Boundary, Typed State, Tool Surface, Multi-Agent Install, Self-Hosting

> **Status note:** this document's original "docs-only, no
> preemptive command edits" framing has been superseded by the
> expanded Lane 0.5 scope. See
> [LANE-0.5-DESIGN-NOTES.md](LANE-0.5-DESIGN-NOTES.md) for the
> authoritative design record and the final execution scope.
> [LANE-0.5-BRIEF.md](LANE-0.5-BRIEF.md) carries the execution
> deliverables list.
>
> The sections below retain the original planning language as a
> historical reference. Where they conflict with the design notes or
> brief, the design notes and brief win.

## Goal

Move workflow mechanics out of shared command markdown and into Tanren code so
installed commands become repo-specific artifacts rather than the canonical
workflow engine.

This lane exists to make Tanren-in-Tanren development viable before Phase 1
automation. The immediate target is **manual self-hosting**: a human invokes
the commands in sequence while Tanren code supplies workflow context and other
mechanics.

This document is the planning contract for that lane. It defines the command
refactor that lane 0.5 must carry out; it does not apply those command changes
in this planning pass.

## Problem Statement

The current methodology layer mixes:

- prompt-local agent instructions
- workflow-engine behavior

That leak appears as:

- literal verification commands in prompts
- issue-tracker shell commands in prompts
- branch / commit / push / PR steps in prompts
- prompts deriving their own task or diff scope

Lane 0.5 documents and isolates that boundary.

## Scope

### In Scope

1. Shared command markdown becomes template/asset material rather than final
   literal workflow instructions.
2. Workflow mechanics are documented as Tanren-code responsibilities.
3. Verification-hook resolution is modeled as command/phase keyed workflow
   resolution, not prompt-local gate literals.
4. Lane docs clearly separate 0.4 from 0.5.
5. Manual self-hosting is documented as the pre-Phase-1 target:
   - shape spec
   - resolve task context
   - run do-task
   - run audit-task
   - run run-demo
   - run audit-spec
   - run walk-spec

### Out of Scope

- runtime / harness implementation
- planner-native orchestration
- API/MCP parity for methodology operations
- final enterprise governance model

## Deliverables

1. Rewrite canon updates:
   - HLD
   - design principles
   - roadmap
   - crate guide
   - methodology boundary doc
2. Methodology doc updates:
   - methodology system
   - phase taxonomy
3. Shared command markdown updates removing prompt-local workflow mechanics
4. New lane 0.5 spec / brief / audit docs

## Required Command Refactor

Lane 0.5 must update the shared command sources so they behave as consumers of
workflow context rather than owners of workflow mechanics.

Required outcomes:

1. `shape-spec`
   - does: collaborative scope, acceptance criteria, demo plan, task plan
   - does not: create/fetch issues itself, choose roadmap candidates itself,
     mutate dependency graphs, prepare branches, or publish
2. `do-task`
   - does: implement the supplied task only
   - does not: discover the next task, choose a gate, or perform SCM actions
3. `audit-task`
   - does: audit the supplied task/diff and emit findings
   - does not: infer its own diff target or mutate workflow state
4. `run-demo`
   - does: execute the supplied demo context and report results
   - does not: decide routing or workflow progression
5. `audit-spec`
   - does: whole-spec audit and fix/defer classification
   - does not: open deferred issues or mutate roadmap/workflow state
6. `walk-spec`
   - does: user-facing validation walkthrough
   - does not: own PR/review/publication steps
7. workflow-heavy commands such as `handle-feedback` and `sync-roadmap`
   - must consume resolved review/work-item context from Tanren-code
   - must stop embedding provider-specific shell workflows directly in prompts

## Acceptance Criteria

1. The tanren-code vs tanren-markdown boundary is explicit and consistent.
2. Lane 0.4 and lane 0.5 scopes are disjoint.
3. Lane 0.5 clearly specifies the shared-command changes required so that the
   commands no longer hardcode:
   - literal verification commands
   - issue-tracker shell commands
   - branch creation / checkout steps
   - commit / push / PR steps
4. Lane 0.5 defines that shared command markdown must refer only to resolved
   workflow context and resolved verification hooks in abstract terms.
5. Manual self-hosting before Phase 1 is documented as the near-term target.
