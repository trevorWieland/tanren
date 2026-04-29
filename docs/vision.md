# Tanren Vision

Tanren is a product-to-proof control plane for agentic software delivery.

Its purpose is to connect product intent to shipped, evidenced behavior. Coding
agents can write code quickly, but Tanren decides what work should exist, why it
matters, what order it should run in, which phase may act next, and what proof
is required before the work counts as complete.

## The Core Chain

Tanren is designed around one continuous chain:

```text
product brief
-> accepted behavior catalog
-> roadmap DAG
-> shaped specs
-> orchestrated implementation
-> BDD evidence and human walk
-> PR, review, merge, ship
-> feedback and proactive analysis
-> updated product plan
```

Each layer preserves meaning for the next layer. The product brief explains why
the product exists. Behaviors define what users, operators, clients, and runtime
actors can do. The roadmap DAG chooses dependency order. Specs turn one roadmap
node into tasks, acceptance criteria, demos, and evidence obligations. The
orchestration loop executes and audits the work. Walks and BDD evidence prove
that the delivered behavior exists.

## What Tanren Is

Tanren is the framework around agent runtimes. It owns product memory,
behavior contracts, roadmap state, workflow state, evidence, policy,
installation, and execution contracts.

Tanren is also an opinionated method. It insists that:

- product behavior is the unit of meaning;
- accepted behaviors and verification status are tracked separately;
- every executable roadmap node completes at least one accepted behavior;
- specs are shaped before implementation;
- product behavior is proven through BDD-style evidence;
- demos show agreed behavior, not just implementation detail;
- agents mutate workflow state through typed tools;
- feedback and analysis update the plan without erasing history.

## What Tanren Is Not

Tanren is not an agent runtime, model provider, editor, ticket tracker, CI
system, or generic task runner. Those systems are replaceable integrations.
Tanren's durable role is deciding what work happens, preserving why it happens,
enforcing how it progresses, and recording what proved completion.

Tanren should be pluggable at integration boundaries and strict about method.
It should not require one issue tracker, source-control provider, model, or
execution substrate. It should require behavior-backed work, shaped specs,
typed state transitions, evidence, audits, and human validation where judgment
matters.

## Why Build It

Autonomous coding fails when it is fast but ungrounded. Tanren is built to
prevent common failures:

- agents implement plausible work that was never tied to product intent;
- tickets are too vague to prove whether completion matters;
- roadmaps are prose lists that go stale instead of executable DAGs;
- tests assert internals but not user-visible behavior;
- reviews inspect diffs without a clear behavior demo story;
- bugs, client requests, and audit findings scatter across tools;
- autonomous loops either interrupt humans constantly or run without guardrails.

Tanren makes autonomous delivery legible, governable, and cumulative.

## Product Method

The intended product method has four layers:

1. **Plan product**: maintain the product brief, users, motivations,
   constraints, success signals, non-goals, assumptions, and open decisions.
2. **Identify behaviors**: maintain a parsable catalog of accepted behaviors
   with product status and verification status.
3. **Craft roadmap**: synthesize accepted behaviors, implementation readiness,
   current progress, dependencies, in-flight work, feedback, and proactive
   analysis into a roadmap DAG.
4. **Execute specs**: shape, orchestrate, audit, adhere, demo, walk, review,
   merge, and update evidence.

The method must work for blank-slate projects and existing codebases. Planning
a new product may start with questions. Adopting Tanren in an existing repo may
start with analysis of current docs, tests, architecture, behaviors, and gaps.

## Execution Loop

The spec loop is the execution layer of the method:

```text
shape-spec
-> do-task
-> task gate
-> audit-task
-> adhere-task
-> investigate and repair if needed
-> spec gate
-> run-demo
-> audit-spec
-> adhere-spec
-> walk-spec
-> review feedback
-> merge
```

Agents perform role-specific work inside a phase. Tanren owns the state
machine, phase capabilities, event log, projected artifacts, task guards,
evidence schemas, and escalation rules.

## Feedback and Proactive Analysis

Tanren treats bugs, client requests, review feedback, support issues,
post-ship outcomes, and scheduled analyses as planning inputs. A report usually
means one of:

- a behavior is missing;
- an existing behavior is misaligned with user intent;
- implementation is incomplete;
- implementation exists but evidence is insufficient;
- the report is false or out of scope.

Scheduled standards sweeps, security audits, dependency audits,
mutation-testing reports, and health checks should produce typed findings or
planning-change proposals. They should not directly mutate active spec tasks.

## Long-Term Empowerment

In its complete state, Tanren should let a team answer:

- What are we building?
- Who is it for?
- Which behaviors are accepted?
- Which behaviors are implemented or asserted?
- What work remains?
- Why is this next?
- What depends on what?
- What is in flight?
- What proved completion?
- What changed after feedback?
- What should we do next?

The core bet is that autonomous coding becomes trustworthy only when it is
embedded in an opinionated product method and a typed evidence-producing
workflow. Tanren turns "agents can write code" into "agents can help deliver a
product whose intended behaviors are planned, implemented, validated, reviewed,
shipped, and continuously improved."

## Current Repository Status

The enacted native command surface is the spec-orchestration loop:

- `shape-spec`
- `do-task`
- `audit-task`
- `adhere-task`
- `run-demo`
- `audit-spec`
- `adhere-spec`
- `walk-spec`
- `handle-feedback`
- `investigate`
- `resolve-blockers`

Temporary project-method bootstrap commands also exist for `plan-product`,
`identify-behaviors`, and `craft-roadmap`. They directly edit planning
artifacts for now and should be replaced by Tanren-native commands once typed
project-method schemas, validators, tools, and events are defined.
