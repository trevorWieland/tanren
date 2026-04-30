---
schema: tanren.product_brief.v0
status: accepted
updated_at: 2026-04-29
---

# Product Brief

## Product Identity

Tanren turns a product idea into enterprise-ready software, with proof at
every step.

Tanren is a product-to-proof control plane for agentic software delivery. It
is the framework around coding agents that preserves product intent, accepted
behaviors, roadmap order, workflow state, execution boundaries, and evidence.
Agent runtimes decide how an assigned role reasons and edits. Tanren decides
what work exists, why it exists, what may run next, and what proof is required
before the work counts as complete.

Enterprise-ready primarily means production-grade engineering discipline:
accepted behavior contracts, executable evidence, standards, CI, auditability,
typed workflow state, isolated execution, and repeatable delivery. Governance,
permissions, secrets, collaboration, feedback intake, and roadmap discipline
support that core.

## Target Users

Tanren is for technical product builders: people who think in long-term
vision, user problems, and product outcomes, while having enough technical
judgment to shape real solutions from those problems.

The initial audience is technical founders, product-minded maintainers, lead
engineers, and small technical teams using coding agents to build real software
without losing quality or product coherence.

The strategic default is team building. Solo use remains first-class because a
solo builder is the smallest useful case of the same product method: the same
planning, behavior, roadmap, evidence, and walk loop should scale from one
builder to a governed team.

## Problems

Coding agents make implementation faster, but fast implementation without
durable product method creates predictable failures:

- vague ideas become plausible code without accepted product intent;
- tickets and roadmaps drift away from user problems;
- tests prove implementation details instead of product behavior;
- demos and reviews lack a clear behavior story;
- bugs, audits, performance findings, and feedback become interruptions rather
  than planned inputs;
- parallel agent work collides without explicit execution boundaries;
- teams cannot explain why work was chosen, what it completed, or what proved
  it.

## Motivations

Tanren exists to make autonomous and semi-autonomous software delivery
legible, governable, cumulative, and product-led.

The product bet is that agentic delivery becomes trustworthy only when it is
embedded in an opinionated method:

```text
product brief
-> accepted behavior catalog
-> roadmap DAG
-> shaped specs
-> orchestrated implementation
-> BDD evidence and human walk
-> PR, review, merge, ship
-> feedback, bug triage, and proactive analysis
-> updated product plan
```

Each layer preserves meaning for the next layer. The product brief explains why
the product exists. Behaviors define what users, operators, clients, and
runtime actors can do. The roadmap DAG chooses dependency order. Specs turn one
roadmap node into acceptance criteria, tasks, demos, and evidence obligations.
The orchestration loop executes and audits work. Walks and BDD evidence prove
that delivered behavior exists.

## Non-Goals

Tanren is not an agent runtime, model provider, editor, ticket tracker, CI
system, generic task runner, or SaaS-first product.

Tanren should not be methodology-neutral. It should be pluggable at the edges,
but strict about behavior-backed work, shaped specs, typed state transitions,
evidence, audits, and human judgment where product direction matters.

Tanren should not replace technical product judgment. It should preserve,
challenge, route, and execute from that judgment without pretending uncertain
product direction is proven fact.

## Constraints

- Product behavior is the unit of meaning.
- Accepted behavior and verification status remain separate facts.
- Every executable roadmap node completes at least one accepted behavior.
- Specs are shaped before implementation.
- Product behavior is proven through BDD-style evidence, including positive and
  falsification witnesses where applicable.
- Agents mutate workflow state through typed tools, not ad hoc edits to
  orchestrator-owned artifacts.
- Execution work must run in explicit, inspectable environments.
- Secrets, credentials, provider access, and execution permissions must remain
  scoped and auditable.
- Integrations are adapter-backed and replaceable.

## Success Signals

Tanren is succeeding when a team can answer from durable state:

- what product is being built and for whom;
- which user problems and success signals justify the current roadmap;
- which behaviors are accepted, implemented, asserted, deprecated, or missing;
- why a roadmap node is next and what it depends on;
- which specs are shaped, running, blocked, awaiting walk, reviewed, or
  shipped;
- which evidence proves each asserted behavior;
- which bugs, audit findings, benchmarks, and post-ship outcomes changed the
  plan;
- where active agent work is running and which harness, credentials, and
  environment boundaries apply.

The early product success bar is self-hosting: Tanren can minimally run its
own full product-to-proof method for Tanren development.

## Core Method

Tanren's product method has four primary layers:

1. **Plan product**: maintain product identity, target users, problems,
   motivations, constraints, non-goals, success signals, assumptions, and open
   decisions.
2. **Identify behaviors**: turn product intent into a parsable catalog of
   accepted user, operator, client, and runtime-actor behaviors with separate
   product and verification status.
3. **Craft roadmap**: synthesize accepted behaviors, implementation readiness,
   current progress, dependencies, findings, and feedback into a
   machine-readable roadmap DAG of spec-sized work.
4. **Execute specs**: shape one roadmap node into a spec, orchestrate tasks,
   run gates and audits, produce behavior evidence, walk the result with a
   human, review, merge, and feed outcomes back into planning.

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

## Feedback And Analysis Funnels

Tanren treats planning, bugs, and automated analysis as first-class funnels
into the same product method.

The central funnel is product planning:

```text
plan-product -> identify-behaviors -> craft-roadmap -> execute specs
```

Bug handling should classify reports into one of the product-method outcomes:

- false alarm or out of scope;
- missing accepted behavior;
- accepted behavior misaligned with user intent;
- behavior implemented but lacking proper executable assertion;
- behavior asserted but currently regressed.

Automated analysis should produce findings or proposed planning changes rather
than directly bypassing the roadmap. Early analysis sources include scheduled
mutation testing, agentic security audits, performance profiling, and
benchmarks.

Feature-request handling is intentionally deferred. Bug handling exercises the
same routing muscles needed for future on-the-fly feature requests: clarify
intent, relate it to behavior, decide whether planning changes, and route
follow-up work with evidence.

## Integrations And Execution Substrates

Initial harness targets are Codex, Claude Code, and OpenCode.

Initial execution substrates are local worktrees and Docker containers, so
parallel specs can run in isolated local environments. Near-follow expansion
targets include Hetzner VMs and GCP VMs.

Initial issue-tracker integrations are Linear and GitHub Issues. Initial source
control integration is GitHub.

These choices are strategic starting points, not core lock-in. Tanren should
keep provider-specific details behind adapters while preserving stable product
method and evidence contracts.

## Tanren-In-Tanren Milestone

The first major product milestone is developing Tanren with Tanren.

Minimum self-hosting means Tanren can use its own method to:

- plan Tanren's product brief;
- identify accepted behaviors;
- craft a roadmap node;
- shape and run a spec through the orchestration loop;
- manage Codex, Claude Code, and OpenCode harness choices;
- manage local worktree and Docker execution environments;
- run parallel specs in isolated environments;
- produce BDD evidence;
- walk accepted work with a human;
- feed bugs, audit findings, performance findings, and shipped outcomes back
  into the behavior catalog and roadmap.

Until this milestone works, the enacted command surface is only a partial
implementation of the product.

## Human Judgment Model

Tanren is built around human product accountability. The ideal human input is
the kind of judgment a strong technical product manager provides: long-term
vision, user-problem framing, product tradeoffs, acceptance judgment, and
enough technical understanding to shape real solution boundaries.

Humans provide primary judgment where product direction matters:

- product identity, target users, problems, and success signals;
- behavior acceptance and rejection;
- roadmap tradeoffs and sequencing;
- spec shaping and non-negotiables;
- walk acceptance;
- sensitive policy, credential, security, and governance decisions;
- resolution of conflicting product direction.

Tanren should automate bounded execution, evidence production, routing,
analysis, and state projection without silently replacing those decisions.

## Open Questions

- What is the smallest useful native project-method schema set for replacing
  the current bootstrap `plan-product`, `identify-behaviors`, and
  `craft-roadmap` commands?
- Which harness and environment capabilities are required before parallel local
  self-hosting is practical rather than only demonstrable?
- What is the first minimal bug-triage artifact shape that can later support
  feature requests without being overfit to bug reports?

## Change Log

- 2026-04-29: Replaced the earlier narrative vision with an accepted
  structured product brief centered on idea-to-enterprise-ready software,
  technical product builders, Tanren-in-Tanren self-hosting, initial
  integrations, and product-method funnels.
