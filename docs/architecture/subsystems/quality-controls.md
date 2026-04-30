---
schema: tanren.subsystem_architecture.v0
subsystem: quality-controls
status: accepted
owner_command: architect-system
updated_at: 2026-04-30
---

# Quality Controls Architecture

## Purpose

This document defines Tanren's quality-control architecture. Quality controls
are the required checks that decide whether active spec work is acceptable
enough to progress through task completion, candidate validation, and
stakeholder review.

Quality controls sit inside orchestration, but they are important enough to
have their own architecture record. They include automated gates, audit
rubrics, standards adherence, autonomous demo critique, guard semantics,
quality-control findings, and the phase taxonomy needed to keep these controls
consistent.

Quality controls do not own behavior assertion. Behavior proof owns BDD
behavior proof and assertion semantics. Quality controls may require proof
links, scenario updates, or proof-relevant checks, but they do not decide that
an accepted behavior is asserted.

## Subsystem Boundary

The quality-controls subsystem owns:

- quality-control phase taxonomy;
- automated task and spec gates;
- rubric audit checks;
- standards adherence checks;
- autonomous run-demo critique semantics;
- task and spec quality-control batches;
- task and spec guard semantics;
- active-spec finding severity and disposition rules;
- audit rubric configuration model;
- standards relevance and criticality rules;
- bounded and redacted control output;
- quality-control read models;
- quality-control event shapes.

The quality-controls subsystem does not own:

- overall spec lifecycle state;
- behavior-proof execution and assertion state;
- post-hoc assessment findings;
- roadmap or planning decisions;
- runtime placement;
- harness adapter mechanics;
- source-control and CI provider mechanics;
- public API/MCP interface contracts;
- observation reports and dashboards.

Orchestration owns when controls run and how passed or failed batches advance
the spec lifecycle. Runtime executes controls inside execution targets.
Provider integrations supply external CI/status signals. Assessment handles
findings outside an active spec loop.

## Core Invariants

1. **Quality controls are required by default.** Gates, audit, adherence, and
   demo critique are core to Tanren's delivery method, not optional garnish.
2. **Controls run through Tanren state.** Control outcomes are typed events and
   read models, not local log parsing or hand-edited files.
3. **Batch before investigation.** A quality-control batch should run all
   required controls, collect all results, and investigate once with the full
   failure context.
4. **Audit and adherence stay separate.** Audit is opinionated quality
   judgment. Adherence is standards compliance.
5. **Numeric audit scoring is intentional.** A 1-10 score is a configurable
   strictness knob for models, teams, and domains; it is not objective truth.
6. **Findings must route.** A blocking or deferred quality finding cannot be
   dropped silently.
7. **Agents do not accept residual risk.** Agents can identify risks, route
   them, or ask for a user decision through blockers. Acceptance is a user or
   policy decision outside autonomous quality-control judgment.
8. **Critical standards are non-deferable.** A critical adherence violation
   blocks until fixed or until a user changes the underlying standard/policy.
9. **Baseline controls are not normally removable.** Projects may tune
   thresholds, profiles, commands, and add controls, but the baseline control
   structure remains in force.
10. **Output is bounded and redacted.** Control logs, model output,
    diagnostics, provider details, and runtime output are sanitized before
    persistence.

## Phase Taxonomy

Tanren phases are described by mode, intent, scope, and autonomy.

Modes:

- `agentic`: a harness-backed agent session performs a bounded assignment;
- `automated`: a deterministic command, tool, or service runs a check;
- `human`: a human-facing walkthrough or decision step occurs.

Intents:

- `planning`: product, behavior, architecture, or roadmap planning;
- `shaping`: turning accepted roadmap nodes into executable specs;
- `changing`: modifying repository code or project artifacts;
- `checking_gate`: deterministic pass/fail verification;
- `checking_audit`: scored quality judgment;
- `checking_adherence`: standards compliance;
- `checking_demo`: autonomous demo critique;
- `validating`: human review or walk;
- `triaging`: diagnosing failure or feedback;
- `resolving`: answering blockers or decisions;
- `operating`: runtime, delivery, or operations work.

Scopes:

- `task`;
- `spec`;
- `roadmap_node`;
- `project`;
- `organization`;
- `installation`;
- `context_dependent`.

Autonomy:

- `autonomous`;
- `interactive`;
- `human_required`.

Quality-controls primarily owns the checking intents. Other phase intents are
documented here only to keep boundaries clear.

## Baseline Controls

Task completion requires this baseline control set:

- `task-gate`;
- `audit-task`;
- `adhere-task`.

Candidate validation requires this baseline control set:

- `spec-gate`;
- `audit-spec`;
- `adhere-spec`;
- `run-demo`.

Projects and organizations may add required controls, tune thresholds, change
commands, select profiles, and add stricter policies. They should not remove
the baseline structure in normal operation.

If an extraordinary policy exception ever disables a baseline control for a
scope, the exception must be explicit, permissioned, visible, audited, and
treated as a governance exception rather than a routine configuration choice.

## Automated Gates

Automated gates run deterministic project commands or services.

Task gates answer whether the task-level check set passes. Spec gates answer
whether the spec-level check set passes.

Gate records include:

- control identifier;
- task or spec scope;
- command or service invoked;
- execution target;
- started and completed timestamps;
- exit status;
- bounded output;
- redaction summary;
- pass/fail status;
- retry metadata;
- source event position.

Gate failure blocks the current batch and routes to investigation after the
batch completes.

## Audit

Audit is opinionated quality judgment against a configurable rubric.

Audit answers: is this good work for the project, spec, and task context?

Audit is scored per rubric pillar on a 1-10 scale. The scale is useful because
teams can tune passing and target thresholds as models and expectations change.
A score is a rubric judgment with rationale and supporting findings, not an
absolute measurement.

Audit rubric records include:

- pillar identifier;
- pillar name;
- scope applicability;
- task and spec descriptions;
- passing score;
- target score;
- enabled/disabled state;
- profile source;
- policy scope.

Audit score records include:

- pillar identifier;
- score;
- rationale;
- supporting finding identifiers;
- auditor actor or worker;
- model or harness metadata where visible;
- source event position.

If a score is below target, it must cite supporting findings. If a score is
below passing, at least one supporting finding must be `fix_now`.

## Audit Profiles

Tanren defines the audit rubric mechanism and broad expected quality
dimensions. Default profiles provide starting pillar sets.

Common default dimensions may include:

- completeness;
- performance;
- scalability;
- security;
- reliability;
- maintainability;
- extensibility;
- style;
- relevance;
- modularity;
- documentation.

Projects can select, clone, tune, add, disable, or replace pillars through
Tanren configuration and policy. For example, a local utility may disable or
lower scalability expectations while keeping strict performance and security
expectations.

The architecture does not freeze one universal pillar list as mandatory for
every project.

## Adherence

Adherence is standards compliance against Tanren-managed standards.

Adherence answers: does this work follow the rules the team codified?

Standards are Tanren-owned state projected into repo-local files. Users may
edit standards through Tanren actions or import standards files through typed
validation flows. Manual edits to generated standards projections are drift.

Standard records include:

- standard identifier;
- name;
- category;
- body;
- applicability rules;
- language or domain tags;
- criticality;
- enabled/disabled state;
- profile source;
- policy scope.

Adherence findings cite the violated standard. A critical standard violation is
`fix_now` and cannot be deferred by an agent.

## Standards Relevance

Adherence controls evaluate only relevant standards.

Relevance inputs may include:

- changed files;
- spec relevance context;
- task scope;
- project languages;
- project domains;
- behavior or interface tags;
- standard applicability rules;
- caller hints that broaden relevance.

Caller hints may broaden relevance but cannot narrow server-derived relevance.
This prevents an agent from excluding a standard by omission.

## Run Demo

`run-demo` is the autonomous practice run for stakeholder-facing behavior.

Run-demo answers: does the spec appear demonstrable before a human walk?

Run-demo may:

- execute the shaped demo flow;
- inspect changed behavior through configured surfaces;
- identify demo blockers;
- identify UX or workflow gaps;
- identify missing proof links;
- identify mismatch between expected and observed behavior;
- record findings for the candidate validation batch.

Run-demo is not walk-spec. It does not create human acceptance and does not
replace stakeholder observation. Walk-spec remains orchestration-owned human
validation.

## Findings

Quality-control findings are active-spec findings.

They are distinct from assessment findings, which exist outside an active spec
loop and route through assessment/planning.

Finding severities:

- `fix_now`: blocks the current control batch and requires remediation;
- `defer`: does not block this checkpoint, but must route to a concrete
  destination;
- `note`: informational, no required route unless policy says otherwise;
- `question`: asks for human clarification only when correctness, scope,
  safety, policy, or acceptance materially depends on the answer.

Questions should be rare. If a question blocks progress, orchestration routes
to `resolve-blockers`. If it does not materially block progress, it should be a
`note`, not a `question`.

## Defer Dispositions

A `defer` finding is invalid without a disposition, rationale, and destination.

Allowed defer dispositions:

- `covered_later_in_spec`: the issue is already planned by a later task in the
  same spec and must reference that task;
- `create_task_in_spec`: the issue is in scope and creates a new task before
  spec completion;
- `linked_existing_spec`: the issue is out of this spec and must reference an
  existing spec or roadmap node;
- `propose_new_spec`: the issue is out of this spec and creates intake or a
  proposal for later shaping.

Deferred findings are not silent backlog. They remain visible in active work,
observation, and assessment where applicable.

## Guard Semantics

A task becomes complete only after implementation and required task controls
pass.

Default task guards:

- implemented;
- gate checked;
- audited;
- adherent.

Candidate validation requires spec controls to pass before the PR moves into
manual review and walk acceptance.

When a mutating task changes after a guard passed, affected guards become stale
and must rerun according to orchestration policy.

## Batch Semantics

Quality-control batches run controls in parallel where runtime policy allows,
but they are evaluated as a batch.

Batch behavior:

- launch all required controls when inputs are ready;
- collect pass/fail/defer/question/timeout results;
- wait for all required results or terminal timeout;
- normalize output;
- redact and bound persisted diagnostics;
- route once to investigation if any blocking result exists;
- advance only when required controls are satisfied.

This prevents wasteful loops where each failed control triggers an isolated
repair while other pending controls would have found related issues.

## Tool And Capability Model

Quality-control agents and automated controls mutate Tanren state only through
typed commands exposed by API/MCP capability scopes.

Capabilities are granted per assignment and phase. Controls receive only the
tools needed for their role, such as reading tasks, recording audit scores,
recording adherence findings, recording gate results, or reporting phase
outcomes.

Out-of-scope tool calls fail with typed permission or capability errors.

The architecture rejects transport-specific parity paths. MCP and API are
network service contracts. CLI/TUI are clients over those contracts, not
fallback state mutation transports.

## Output Redaction

Quality-control output may include logs, model text, command output, provider
metadata, source snippets, and runtime diagnostics.

Before persistence or display, output must be:

- size-bounded;
- secret-redacted;
- permission-filtered;
- source-linked where useful;
- labeled with truncation or omission metadata;
- stored separately from canonical event payloads when large.

Secret values must never appear in findings, scores, gate outputs, demo
records, audit exports, reports, or repo-local projections.

## Events

Quality controls emit typed events for:

- control batch started, completed, failed, or cancelled;
- automated gate started, passed, failed, timed out, or retried;
- audit run started, completed, failed, or timed out;
- audit score recorded;
- audit profile selected or changed;
- adherence run started, completed, failed, or timed out;
- adherence finding recorded;
- standards relevance evaluated;
- run-demo started, completed, failed, or timed out;
- quality finding recorded, routed, resolved, or superseded;
- defer disposition recorded or fulfilled;
- guard satisfied, invalidated, or reset.

Events include bounded identifiers, scope, actor, source position, and redacted
diagnostics. Events do not include secret values.

## Read Models

Required quality-control read models include:

- active control batch status;
- task guard status;
- spec candidate validation status;
- gate result history;
- audit profile and effective rubric;
- audit scores by task/spec/pillar;
- adherence standards relevance;
- adherence findings by standard;
- run-demo result summary;
- active quality findings;
- defer routing status;
- stale guard status;
- quality-control configuration by scope.

Read models include freshness, redaction, source position, and policy metadata
where applicable.

## Accepted Decisions

- Quality controls are a dedicated subsystem rather than being folded entirely
  into orchestration.
- Gates, audit, adherence, and run-demo are required baseline controls.
- Quality-control batches collect all required results before investigation.
- Audit uses configurable 1-10 rubric scoring.
- Architecture defines the rubric mechanism and broad dimensions, while
  profile content remains configurable.
- Audit and adherence remain separate.
- Standards are Tanren-owned state with repo-local projections.
- Critical standards are non-deferable by agents.
- `defer` requires disposition, rationale, and destination.
- Agents do not accept residual risk.
- Blocking questions route to resolve-blockers and should be rare.
- Behavior proof remains adjacent and owns assertion semantics.
- Quality-control output is bounded and redacted before persistence.
- Default audit profiles are `balanced`, `strict-production`,
  `local-utility`, and `library`.
- Default audit passing score is 7/10 and target score is 8/10 unless a
  profile sets stricter thresholds. `strict-production` passes at 8/10 and
  targets 9/10.
- Baseline controls are not agent-disableable. Any extraordinary disable path
  requires explicit user approval through `resolve-blockers`.
- Baseline control types are gates, audit, adherence, and demo critique.
  Additional control types require an architecture update before becoming
  product contract.
- Shared quality findings include source control, source phase, severity,
  provenance, affected scope, affected behavior/resource where known,
  rationale, disposition, destination, source references, freshness, and
  redaction state.
- Mutating controls run in the active execution target. Read-only gates, audit,
  adherence, and demo critique may run in cloned equivalent targets when the
  configured execution strategy permits it.

## Rejected Alternatives

- **Quality controls embedded only in orchestration.** Rejected because gates,
  audit, adherence, demo critique, findings, and guard semantics need a focused
  architecture record.
- **Pass/fail-only audit.** Rejected because numeric rubric scoring gives teams
  a tunable strictness knob.
- **Audit and adherence as one check.** Rejected because quality judgment and
  standards compliance answer different questions.
- **Dropping deferred findings into an unstructured backlog.** Rejected because
  deferred findings need explicit destination and traceability.
- **Agent-accepted residual risk.** Rejected because accepting risk is a user
  or policy decision, not an autonomous quality-control outcome.
- **Removable baseline controls as normal configuration.** Rejected because
  Tanren's trust model depends on gate, audit, adherence, and demo controls.
- **Repo-local standards as canon.** Rejected because standards need typed
  state, policy, auditability, and projection regeneration.
