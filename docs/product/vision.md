---
schema: tanren.product_brief.v0
status: accepted
updated_at: 2026-04-29
owner_command: plan-product
---

# Product Vision

## Product Identity

Tanren turns product intent into behavior-backed software delivery with proof at
every step.

Tanren is a product-to-proof control plane for agentic software delivery. It
preserves product intent, accepted behaviors, roadmap order, workflow state,
execution boundaries, source signals, and behavior proof. Agent runtimes decide how an assigned role
reasons and edits. Tanren decides what work exists, why it exists, what may run
next, and what proof is required before work counts as complete.

Enterprise-ready primarily means production-grade engineering discipline:
accepted behavior contracts, executable behavior proof, standards, CI, auditability,
typed workflow state, isolated execution, and repeatable delivery.

## Target Users

Tanren is for technical product builders: people who think in long-term vision,
user problems, and product outcomes, while having enough technical judgment to
shape real solutions from those problems.

The initial audience is technical founders, product-minded maintainers, lead
engineers, and small technical teams using coding agents to build real software
without losing quality or product coherence.

The strategic default is team building. Solo use remains first-class because a
solo builder is the smallest useful case of the same method.

## Problems

Coding agents make implementation faster, but fast implementation without a
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

Tanren exists to make autonomous and semi-autonomous software delivery legible,
governable, cumulative, and product-led.

The product bet is that agentic delivery becomes trustworthy only when it is
embedded in an opinionated method:

```text
product vision
-> accepted behavior catalog
-> system architecture
-> implementation assessment
-> roadmap DAG
-> shaped specs
-> orchestrated implementation
-> behavior proof and human walk
-> PR, review, merge, ship
-> feedback, bug triage, and proactive analysis
-> updated product plan
```

Each layer preserves meaning for the next layer. Product vision explains why
the product exists. Behaviors define what users, operators, clients, and
runtime actors can do. Architecture chooses how the system will be built.
Implementation assessment reports what is currently true. The roadmap DAG
chooses dependency order. Specs turn one roadmap node into acceptance criteria,
tasks, demos, and proof obligations. The orchestration loop executes and
audits work. Walks and behavior proof prove delivered behavior.

## Non-Goals

Tanren is not an agent runtime, model provider, editor, ticket tracker, CI
system, generic task runner, or SaaS-first product.

Tanren should not be methodology-neutral. It should be pluggable at the edges,
but strict about behavior-backed work, shaped specs, typed state transitions,
source-linked assessments, audits, and human judgment where product direction
matters.

Tanren should not replace technical product judgment. It should preserve,
challenge, route, and execute from that judgment without pretending uncertain
product direction is proven fact.

## Constraints

- Product behavior is the unit of meaning.
- Accepted behavior and verification status remain separate facts.
- Every executable roadmap node completes at least one accepted behavior.
- Specs are shaped before implementation.
- Product behavior is proven through BDD-style behavior proof, including positive and
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
- which specs are shaped, running, blocked, awaiting walk, reviewed, or shipped;
- which behavior proof proves each asserted behavior;
- which bugs, audit findings, benchmarks, and post-ship outcomes changed the
  plan;
- where active agent work is running and which harness, credentials, and
  environment boundaries apply.

The early product success bar is self-hosting: Tanren can minimally run its own
full product-to-proof method for Tanren development.

## Core Method

Tanren's planning and execution method has six primary layers:

1. **Plan product**: maintain product identity, users, problems, motivations,
   constraints, non-goals, success signals, assumptions, and open decisions.
2. **Identify behaviors**: maintain a parsable catalog of accepted user,
   operator, client, and runtime-actor behaviors with separate product and
   verification status.
3. **Architect system**: choose the implementation strategy, system boundaries,
   technology posture, delivery model, operations posture, and subsystem design.
4. **Assess implementation**: inspect current code, tests, docs, and source signals
   to report which accepted behaviors appear implemented, asserted, missing,
   stale, or uncertain.
5. **Craft roadmap**: synthesize behaviors, architecture, implementation state,
   current progress, dependencies, findings, and feedback into a
   machine-readable roadmap DAG of spec-sized work.
6. **Execute specs**: shape one roadmap node into a spec, orchestrate tasks,
   run gates and audits, produce behavior proof, walk the result with a
   human, review, merge, and feed outcomes back into planning.

## Feedback And Analysis Funnels

Tanren treats planning, bugs, and automated analysis as first-class funnels
into the same product method.

Bug handling should classify reports into one of the product-method outcomes:

- false alarm or out of scope;
- missing accepted behavior;
- accepted behavior misaligned with user intent;
- behavior implemented but lacking proper executable assertion;
- behavior asserted but currently regressed.

Automated analysis should produce findings or proposed planning changes rather
than directly bypassing the roadmap. Early analysis sources include scheduled
mutation testing, agentic security audits, performance profiling, standards
sweeps, and benchmarks.

## Human Judgment Model

Tanren is built around human product accountability. Humans provide primary
judgment where product direction matters:

- product identity, target users, problems, and success signals;
- behavior acceptance and rejection;
- architecture tradeoffs that change product or operational posture;
- roadmap tradeoffs and sequencing;
- spec shaping and non-negotiables;
- walk acceptance;
- sensitive policy, credential, security, and governance decisions;
- resolution of conflicting product direction.

Tanren should automate bounded execution, behavior proof production, routing,
analysis, and state projection without silently replacing those decisions.

## Product Defaults

- Native project-method state starts with typed product, behavior,
  architecture, roadmap, assessment, and spec records.
- Harness and environment capability support is designed for containerized or
  remote execution from the start, with serial execution as the safe default
  and parallel execution as policy-controlled optimization.
- Intake artifacts are classified before planning mutation as behavior gap,
  behavior misalignment, assertion gap, implementation regression, provider
  issue, operational issue, or false positive.
