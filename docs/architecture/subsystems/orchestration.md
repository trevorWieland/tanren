---
schema: tanren.subsystem_architecture.v0
subsystem: orchestration
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Orchestration Architecture

## Purpose

This document defines Tanren's spec orchestration architecture. Orchestration
turns one accepted roadmap node into shaped, implemented, checked,
behavior-proven, walked, reviewed, and merge-ready work.

Orchestration is not product planning, implementation assessment, runtime
placement, behavior-proof semantics, source-control provider mechanics, or release
operations. It coordinates the active spec loop and routes work until the spec
is accepted and ready for source-control integration to merge.

## Subsystem Boundary

The orchestration subsystem owns:

- spec lifecycle state;
- task lifecycle state;
- phase taxonomy for active spec work;
- task guard batches;
- candidate validation batches;
- active-spec findings;
- investigation and routing loops;
- blocker escalation;
- walk-spec acceptance;
- human code-review approval state as an orchestration input;
- team coordination for active specs;
- merge-ready handoff to source-control integration.

The subsystem does not own roadmap creation, behavior acceptance, architecture
decisions, implementation assessment, spec-independent analysis, worker
placement, harness execution, source-control provider mechanics, CI provider
mechanics, merge execution, release state, or behavior-proof semantics.

## Core Invariants

1. **Orchestration starts from an accepted roadmap node.** `shape-spec` is not
   product planning. Ideas, issues, bugs, and feedback must first route through
   planning or assessment into a roadmap node.
2. **One roadmap node shapes into one active spec.** A spec is the execution
   unit for implementation, proof, walk, review, and merge-ready handoff.
3. **Every project has a mergeable source-control provider.** Draft PR
   creation is mandatory because external CI, automated review, branch status,
   and human review need a candidate-change object.
4. **Task completion is guarded.** A task becomes complete only after
   implementation and required task-level checks pass.
5. **Task completion is terminal.** Completed tasks are not reopened. Later
   findings create new tasks.
6. **Check failures are batched before repair.** Tanren waits for all checks in
   a task or candidate validation batch before dispatching investigation.
7. **Investigation and routing is shared.** Gate failures, audits, adherence,
   proof failures, CI failures, automated reviews, human review comments,
   walks, merge conflicts, and intent drift use one routing model.
8. **Draft PR precedes candidate validation.** After all task guards pass,
   Tanren creates a draft PR and then runs internal spec checks and external
   automated checks together.
9. **Ready-for-review means automated checks are clean.** Human walk and code
   review begin after required internal and external automated checks pass.
10. **Walk and code review are separate acceptance channels.** Walk-spec proves
    behavior to stakeholders. Code review inspects implementation. Policy
    decides required counts and eligible reviewers.
11. **Behavior-affecting changes stale walk acceptance.** Any change after a
    walk acceptance invalidates that acceptance unless policy classifies the
    change as non-behavioral.
12. **Source-control integration owns merge execution.** Orchestration emits
    merge-ready handoff; source-control integration executes the merge and
    delivery/release subsystems consume the merged result.

## Canonical Flow

```text
accepted roadmap node
-> shape-spec
-> task loop
-> task check batch
-> draft PR
-> candidate validation batch
-> ready for review
-> required walk-spec acceptances + human code-review approvals
-> merge-ready handoff
-> source-control merge
```

## Spec Lifecycle

Spec lifecycle states are:

- **candidate**: a roadmap node has been selected for shaping;
- **shaping**: `shape-spec` is defining scope, acceptance, tasks, proof, demo,
  review policy, and dependencies;
- **ready**: the shaped spec can start implementation;
- **running**: task execution is active;
- **blocked**: the spec awaits human resolution, dependency completion, policy
  approval, or external unblock;
- **candidate_validation**: all task guards passed, draft PR exists, and
  internal/external automated checks are running or being repaired;
- **ready_for_review**: required automated checks have passed and manual walk
  and code-review acceptance can proceed;
- **awaiting_acceptance**: one or more required walks or code reviews remain;
- **merge_ready**: required walks and code reviews are satisfied;
- **handed_to_integration**: source-control integration has accepted
  merge-ready handoff;
- **cancelled**: work stopped before completion;
- **archived**: work preserved without active merge.

Spec state is event-sourced. Repo-local spec documents are projections.

## Shape-Spec

`shape-spec` is an interactive phase. It turns one accepted roadmap node into
a spec.

Shaped spec state includes:

- linked roadmap node;
- accepted behaviors completed by the spec;
- problem statement and scope;
- non-goals;
- acceptance criteria;
- task plan;
- behavior-proof obligations;
- demo path for `run-demo` and `walk-spec`;
- required task guards;
- required candidate validation checks;
- required walk and code-review acceptance policy;
- dependencies and base branch;
- expected source-control target.

`shape-spec` may ask clarifying questions and may route gaps back to planning,
but it must not silently create product direction outside the roadmap node.

## Task Lifecycle

Tasks are autonomous units of work inside one spec.

Task states are:

- **pending**: task exists but has not started;
- **in_progress**: `do-task` or repair work is active;
- **implemented**: implementation is complete but guards have not all passed;
- **checking**: task guard batch is running or being summarized;
- **complete**: required guards passed;
- **abandoned**: task is no longer pursued and has a typed disposition.

Task rules:

- `do-task` implements one task slice.
- A non-terminal task may be revised during investigation.
- A completed task is terminal.
- Findings against incomplete work update or repair the current task.
- Findings after task completion create new tasks.
- Abandoned tasks require a typed reason and replacement or explicit discard
  disposition.
- Every task has typed origin provenance such as shape-spec, investigation,
  audit, adherence, demo, review feedback, walk rejection, merge conflict,
  intent drift, or user request.

## Task Check Batch

After `do-task` reports implementation, Tanren runs the task check batch.

The required baseline checks are:

- `task-gate`: automated task-scoped verification command;
- `audit-task`: autonomous rubric-based code-quality audit;
- `adhere-task`: autonomous standards-compliance check.

These checks are first-class method phases. They may be configured, extended,
or policy-controlled, but they are not optional in the baseline method because
they are where Tanren checks task-level correctness, quality, and standards
before allowing progress.

Tanren waits for all task-check results before dispatching investigation. If
any check fails, one investigation receives the full batch so repair work can
address all known failures together.

## Draft PR Creation

When every task is complete and task guards have passed, orchestration requests
draft PR creation.

Draft PR creation:

- is mandatory for every spec;
- creates or updates the candidate source-control object;
- signals external CI and automated code-review systems;
- records the branch, base branch, source-control provider, and PR identity;
- moves the spec into candidate validation.

Source-control integration owns the source-control mechanics. Orchestration
owns the state transition and consumes provider status.

## Candidate Validation Batch

After the draft PR exists, Tanren runs internal spec checks and consumes
external automated checks as one candidate validation batch.

Internal checks:

- `spec-gate`: automated spec-scoped verification command;
- `audit-spec`: autonomous rubric-based whole-spec audit;
- `adhere-spec`: autonomous whole-spec standards-compliance check;
- `run-demo`: autonomous critic/practice demo run.

External automated checks:

- source-control CI and workflow status;
- automated AI code review;
- provider status checks;
- mergeability checks;
- configured branch or PR protection checks.

Tanren waits for the required candidate validation results before dispatching
repair. If any required internal or external check fails, investigation
receives the whole batch and routes repair.

This Option A ordering is intentional: the draft PR starts external feedback
early, while draft status keeps the candidate out of human review until
Tanren's internal and external automated checks pass.

## Run-Demo And Walk-Spec

`run-demo` and `walk-spec` are separate phases.

`run-demo` is autonomous. It is an internal critic and practice run that checks
whether the spec is demoable and whether completed behaviors appear to work as
intended before human attention is requested.

`walk-spec` is interactive. It is a live stakeholder demo and acceptance flow.
It re-demonstrates the behavior with human judgment and does not blindly trust
or consume `run-demo`.

Passing `run-demo` is part of candidate validation. It is not a substitute for
a required walk.

## Ready For Review

A spec becomes ready for review when:

- all task guards passed;
- draft PR exists;
- required internal spec checks passed;
- required external automated checks passed;
- no unresolved candidate-validation investigation remains;
- source-control provider reports the candidate is reviewable.

Ready-for-review changes the PR from draft to ready-for-review and opens the
manual acceptance stage.

## Manual Acceptance

Manual acceptance consists of separate channels:

- **walk acceptance** through `walk-spec`;
- **code-review approval** through source-control review or configured review
  integration.

Project or organization policy defines:

- required walk count;
- eligible walk acceptors;
- required code-review approval count;
- eligible code reviewers;
- whether approvals are personal, project, or organization scoped;
- whether certain low-risk scopes can omit code review;
- whether any approval requires additional policy approval.

Any actionable review comment moves the spec and PR back to draft
candidate-checking. Any code change after walk acceptance makes prior walk
acceptance stale unless policy explicitly classifies the change as
non-behavioral.

## Investigation And Routing

Investigation is the shared repair and routing loop for active specs.

Triggers include:

- task gate failure;
- task audit finding;
- task adherence finding;
- spec gate failure;
- spec audit finding;
- spec adherence finding;
- run-demo failure;
- external CI failure;
- automated AI review finding;
- human code-review comment;
- walk rejection;
- merge conflict;
- base-branch intent drift;
- provider status failure;
- runtime or harness failure affecting the active spec.

Investigation outcomes include:

- revise a non-terminal task;
- create a new task;
- mark feedback addressed, invalid, or already satisfied;
- route out-of-scope material to assessment or planning;
- request more information;
- escalate to `resolve-blockers`;
- cancel or archive the spec according to policy.

Task-scoped failures repair the same non-terminal task. Spec-scoped failures
and feedback after completed tasks create new tasks. Completed tasks are not
reopened.

## Resolve-Blockers

`resolve-blockers` is an interactive escalation phase.

It is used when autonomous investigation cannot safely proceed because product
direction, scope, policy, credential access, dependency state, external system
state, or reviewer intent requires human judgment.

Resolution may unblock work, revise tasks, create tasks, route to planning,
cancel work, or change the spec disposition according to permissions and
policy.

## Base-Branch And Intent Drift

Orchestration tracks base-branch changes while a spec is active.

When another branch merges into the base branch:

- if text merge conflicts exist, the spec routes to merge-conflict
  investigation;
- if no text conflict exists, the spec still pauses merge readiness for
  intent-drift investigation.

Intent-drift investigation checks whether newly merged work changes the
meaning or completeness of this spec. A branch can become incomplete without a
textual conflict when parallel work changes the product surface the spec was
meant to cover.

For stacked-diff or dependent specs, orchestration tracks dependency branches,
base revisions, and readiness to revalidate when upstream work changes.
Source-control integration owns the source-control mechanics of rebasing and
merging.

## Active-Spec Findings

Active-spec findings are scoped to the current spec and candidate branch. They
represent incomplete implementation, quality gaps, standards violations, demo
failures, review comments, merge conflicts, or active-spec risks.

They are distinct from assessment findings, which are spec-independent and may
become one or more future specs. Active-spec findings route inside the current
spec unless they are explicitly out of scope.

The finding model should share common fields with assessment findings where
useful, such as severity, provenance, source-state, source, scope, and
rationale, but active
spec findings have task/spec provenance and repair routing.

## Team Coordination

Orchestration owns coordination around active specs:

- current owner;
- assignee;
- reviewer;
- walk acceptor;
- takeover request;
- assist permissions;
- handoff notes;
- blocker responder;
- duplicate or overlapping active work detection;
- visibility of active team work.

Coordination decisions are policy-checked and event-sourced. Ownership is
accountability and routing; it is not automatically broad permission to make
every decision.

## Handoff To Source-Control Integration

When required walk acceptances and code reviews are satisfied, orchestration
emits a merge-ready handoff.

The handoff includes:

- spec identity;
- roadmap node identity;
- accepted behavior IDs;
- PR identity;
- source branch and base branch;
- required checks and approval summary;
- proof status summary;
- known residual risks;
- merge and release constraints.

Source-control integration owns merge execution. Delivery and release-related
subsystems consume the merged result for shipped state, post-merge cleanup, and
post-release learning handoff.

## Accepted Orchestration Decisions

- Orchestration starts from an accepted roadmap node.
- `shape-spec` is not product planning.
- Every spec creates a draft PR after all tasks complete and task guards pass.
- Draft PR creation precedes candidate validation.
- Internal spec checks and external automated checks run as one candidate
  validation batch.
- Task checks and candidate validation checks are batched before
  investigation.
- `task-gate`, `audit-task`, and `adhere-task` are first-class required
  task-level phases.
- `spec-gate`, `audit-spec`, `adhere-spec`, and `run-demo` are first-class
  spec-level validation phases.
- `run-demo` is an autonomous critic/practice run.
- `walk-spec` is a separate human/stakeholder demo and acceptance phase.
- Human code review and walk acceptance are separate acceptance channels.
- Actionable review feedback moves the PR/spec back to draft
  candidate-checking.
- Code changes after walk acceptance stale prior walk acceptance by default.
- Base-branch changes trigger merge-conflict or intent-drift investigation.
- Task-complete is terminal.
- Task-scoped failures repair non-terminal tasks; spec-scoped failures create
  new tasks.
- Source-control integration owns merge execution after orchestration emits
  merge-ready handoff.
- Review policy defines required walk count, required code-review count,
  required reviewer scopes, stale-on-change rules, and merge authority.
- Required external checks are project configured. Source-control mergeability
  is always required.
- Post-walk changes preserve walk acceptance only when marked documentation,
  metadata, generated projection refresh, or policy-approved non-behavioral
  change with changed paths and rationale.
- Shared findings include source control, source phase, severity, provenance,
  source-state, affected scope, affected behavior/resource where known,
  rationale, disposition, destination, source references, freshness, and
  redaction state.
- Tanren requests draft/ready transitions through the source-control provider
  and records provider state as source signals; it does not treat provider
  state as canonical orchestration state.

## Rejected Alternatives

- **Starting orchestration from an unplanned draft spec.** Rejected because
  `shape-spec` should not become product planning.
- **Creating PRs only after walk acceptance.** Rejected because CI, automated
  review, and provider checks need to run before humans spend final acceptance
  attention.
- **Running internal spec checks before draft PR creation.** Rejected for the
  baseline because early draft PR creation improves feedback throughput and the
  cost of changing the ordering later is low.
- **Treating run-demo as walk acceptance.** Rejected because autonomous critic
  runs cannot replace stakeholder judgment where policy requires a walk.
- **Treating code review as walk acceptance.** Rejected because code inspection
  and behavior demonstration answer different questions.
- **Reopening completed tasks.** Rejected because monotonic task history keeps
  repair provenance clear.
- **Ignoring clean base-branch merges.** Rejected because intent drift can
  occur without text conflicts.
