---
schema: tanren.subsystem_architecture.v0
subsystem: assessment
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Assessment Architecture

## Purpose

This document defines Tanren's assessment subsystem. Assessment is the
post-hoc understanding layer that determines what appears true about the
current project, which signals matter, which findings are uncertain, and how
new information should route back into planning or delivery.

Assessment is not the same as orchestration testing, spec gates, CI checks, or
behavior proof execution. It can consume those results, but it does not own
the active spec lifecycle or required PR gate flow.

## Subsystem Boundary

The assessment subsystem owns:

- implementation assessment against accepted behaviors;
- behavior coverage and assertion classification read models;
- uncertainty, stale, and disputed assessment review;
- spec-independent analysis run semantics and result records;
- bug, feedback, dependency, security, benchmark, audit, and field-report
  intake classification;
- normalized findings and recommendations from external tools and adapters;
- severity, provenance, source-state, affected-context, and routing metadata;
- routing decisions into planning proposals, roadmap revisions, draft spec
  candidates, assertion work, implementation repair, or no action;
- staleness indicators for assessment results.

The assessment subsystem does not own scheduling mechanics, runtime placement,
worker assignment, proof execution, CI gate enforcement, active spec
orchestration, planning acceptance, or release operations. Runtime and
operations decide when and where assessment work runs. Planning decides
whether proposed product direction changes are accepted.

## Core Invariants

1. **Assessment reports what appears true.** It does not directly change
   accepted product behavior, architecture, or roadmap canon.
2. **Assessment is proof- and source-linked.** Classifications, findings, and
   recommendations cite visible behavior proof, source signals, or rationale.
3. **Behavior acceptance is separate from assessment.** A behavior may be
   accepted even when missing, uncertain, unasserted, stale, or regressed.
4. **Assertion state belongs to assessment and behavior proof, not behavior
   records.** A behavior record describes product intent; assessment and
   behavior proof report whether implementation appears asserted.
5. **Uncertainty remains visible.** Assessment must not hide ambiguity by
   choosing the most optimistic classification.
6. **Findings may begin unlinked.** Some signals imply a missing behavior or
   product gap. Other assessment modes, such as behavior-by-behavior
   implementation assessment, require behavior links.
7. **Assessment routes, it does not silently mutate.** Product direction
   changes become planning proposals. Clear implementation repairs may become
   draft spec candidates, but they still pass through shaping and policy.
8. **Spec-independent analysis is first-class.** Long-running or scheduled
   analysis such as mutation testing, security scans, dependency review, and
   benchmarks can influence planning without becoming PR gates.
9. **External payloads are normalized.** Provider-specific source payloads are
   retained only where policy permits; assessment stores Tanren-level
   categories and routing state.
10. **Staleness errs conservative.** Assessment may mark results potentially
    stale when relevant context changes; it should avoid overclaiming that a
    result is definitely current.

## Assessment Sources

Assessment consumes signals from:

- implementation assessment commands;
- BDD behavior proof outcomes;
- CI and source-control status;
- mutation testing;
- dependency and supply-chain alerts;
- security scans and agentic security reviews;
- performance profiling and benchmarks;
- standards sweeps and codebase-wide audits;
- customer bug reports;
- client feedback;
- support, customer success, field engineering, and operator reports;
- post-release health checks;
- runtime incidents and operational failures;
- external provider webhooks and API polling.

These sources may be manual, scheduled, webhook-triggered, or event-triggered.
Scheduling and execution placement are runtime and operations concerns.
Assessment owns the run, result, finding, recommendation, and routing semantics.

## Assessment Runs

An assessment run records a bounded assessment activity.

Run metadata includes:

- run ID;
- project scope;
- source type;
- trigger: manual, scheduled, webhook, event, or provider poll;
- initiating actor or service account;
- runtime assignment reference where applicable;
- analysis type;
- input references;
- start and finish time;
- status;
- non-secret proof or source references;
- summary;
- staleness status.

Runs may be lightweight, such as ingesting a bug report, or expensive, such as
a mutation test sweep. Long-running runs execute under runtime placement and
credential policy.

## Behavior Assessment

Behavior assessment classifies accepted behavior against current
implementation, behavior proof, and known signals.

Behavior assessment classifications are:

- **not_assessed**: no current assessment exists;
- **missing**: implementation or behavior proof appears absent;
- **implemented**: implementation appears to support the behavior, but active
  behavior proof is missing or insufficient;
- **asserted**: active executable behavior proof appears to support the
  behavior under behavior-proof policy;
- **stale**: prior assessment or assertion may no longer reflect current
  implementation, dependencies, configuration, or runtime context;
- **regressed**: a previously implemented or asserted behavior appears broken;
- **uncertain**: proof or source signals are conflicting, incomplete,
  insufficiently supported, stale, or not visible enough to classify safely.

Each behavior assessment records rationale, proof/source references,
provenance, source-state, freshness, source, and assessment time. Behavior
assessment does not mutate the behavior record.

Assessment modes that exist specifically to evaluate behavior coverage must
link findings and classifications to behavior IDs. Broader intake and analysis
modes may produce unlinked findings when they suggest a behavior gap,
architecture gap, dependency risk, or operational issue.

## Findings And Recommendations

Assessment normalizes source signals into findings and recommendations.

Finding metadata includes:

- source run or intake item;
- source adapter or provider;
- affected project scope;
- affected behavior, planning record, architecture record, dependency,
  provider, runtime target, proof/source item, or unknown context;
- severity;
- provenance;
- source-state;
- classification;
- non-secret proof or source references;
- suggested route;
- current routing state;
- staleness status.

Severity describes impact. Provenance, source-state, and freshness describe how
well the finding is supported. Uncertain or insufficient-source findings should
route to review or investigation rather than accepted work.

Recommendations are suggested next actions derived from findings. They may be
accepted, rejected, deferred, routed for investigation, or converted into
planning proposals or draft spec candidates according to policy.

## Intake Classification

Assessment classifies bug reports, support reports, client feedback, external
tickets, and post-release signals before they affect accepted planning state.

Core classifications include:

- false alarm;
- out of scope;
- missing accepted behavior;
- accepted behavior misaligned with user intent;
- behavior implemented but lacking proper executable assertion;
- asserted behavior regression;
- implementation repair without product change;
- dependency, provider, runtime, or configuration issue;
- needs more investigation.

The classification records rationale and links to affected behavior or
planning context when known. If no accepted behavior exists, the report can
route to a behavior-gap planning proposal.

## Spec-Independent Analysis

Some valuable analysis is too expensive, broad, or asynchronous to act as a
required spec or PR gate. Assessment treats these as spec-independent analysis
sources.

Examples include:

- mutation testing;
- weekly security scanning;
- dependency and supply-chain review;
- broad performance profiling;
- benchmark trend analysis;
- whole-codebase standards sweeps;
- post-release health checks;
- customer feedback aggregation;
- field reports about client environments.

Results from these analyses produce findings and recommendations. They do not
automatically fail active specs or block PRs unless a separate policy routes
the finding into delivery or operations.

## Routing

Assessment routes findings and recommendations into the product method.

Supported routes include:

- no action with rationale;
- false alarm or out-of-scope closure;
- planning proposal for behavior, product brief, architecture, roadmap,
  assumption, or decision change;
- draft spec candidate for implementation repair or assertion work;
- roadmap revision candidate;
- behavior proof or assertion work;
- investigation request;
- runtime or operations incident follow-up;
- credential, provider, or configuration follow-up.

Assessment may create draft spec candidates for clear repair or assertion
needs. It must not skip `shape-spec` by default. Future automation may mature
enough to shape some specs without a human, but the baseline architecture keeps
spec shaping as the controlled transition into delivery.

## External Tool And Provider Normalization

External tools and providers have source-specific payloads. Assessment stores
Tanren-level normalized records and links provider-specific source artifacts
where policy permits.

Adapter examples include:

- GitHub Actions or other CI providers;
- Dependabot or dependency scanners;
- security scanners;
- mutation testing runners;
- benchmark systems;
- issue trackers;
- customer feedback systems;
- incident or monitoring systems.

Provider adapters own fetching, authentication, pagination, provider-specific
status parsing, and source payload capture. Assessment owns normalized
classification, severity, provenance, source-state, routing, and
behavior/planning linkage.

## Relationship To Behavior Proof

Assessment consumes behavior proof outcomes. It does not own proof execution.

BDD proof, demos, audit results, runtime summaries, CI reports, external tool
payloads, and human review notes are source records owned by behavior proof,
orchestration, integrations, runtime, operations, or delivery. Assessment
references them to classify behavior status and route findings.

Assessment may report that a behavior is asserted when active behavior proof
policy is satisfied. It may report that assertion is stale, missing, or
regressed when later signals undermine proof freshness or source support.

## Relationship To Planning

Assessment is a major input to the planning alteration funnel.

Assessment can produce:

- planning proposals;
- stale-assumption signals;
- behavior-gap reports;
- architecture-gap reports;
- roadmap revision candidates;
- decision review prompts.

Planning owns acceptance of product direction changes. Assessment detects and
explains why a change might be needed.

## Staleness

Assessment results may become stale when relevant context changes.

Potential staleness triggers include:

- code changes;
- dependency changes;
- configuration changes;
- architecture changes;
- behavior changes;
- behavior-proof policy changes;
- runtime environment changes;
- provider authorization or capability changes;
- elapsed time beyond an analysis-specific freshness window.

Staleness should err toward "potentially stale" rather than claiming exact
invalidity. A stale result remains historically useful but should not be used
as current proof without review or reassessment.

## Audit And Events

Assessment state is event-sourced. Events include:

- assessment run requested, started, completed, failed, or cancelled;
- source signal ingested;
- behavior classified;
- finding created, updated, linked, unlinked, confirmed, rejected, deferred, or
  marked stale;
- recommendation created, accepted, rejected, deferred, or routed;
- uncertainty reviewed or resolved;
- intake item classified;
- draft spec candidate created;
- planning proposal emitted from assessment;
- assessment result marked potentially stale.

Assessment events never include secret values or hidden provider payloads.
They reference behavior proof or source records according to visibility policy.

## Accepted Assessment Decisions

- Assessment is distinct from orchestration gates, CI required checks, and
  proof execution.
- Assessment owns implementation assessment semantics and spec-independent
  analysis result semantics.
- Scheduling and runtime placement for analyses are runtime and operations
  responsibilities.
- Behavior assessment classifications include `not_assessed`, `missing`,
  `implemented`, `asserted`, `stale`, `regressed`, and `uncertain`.
- `asserted` belongs to assessment and behavior-proof state, not behavior
  records.
- Findings carry severity, provenance, source-state, and freshness metadata.
- Findings may be unlinked when they imply behavior gaps or unknown context.
- Behavior-focused assessment runs must link findings to behavior IDs.
- External tools are normalized through adapters, with raw/source-specific
  payloads retained only where policy permits.
- Assessment routes product-direction changes through planning proposals.
- Assessment may create draft spec candidates but does not skip `shape-spec`
  by default.
- Spec-independent analysis can influence planning without becoming a PR gate.
- Staleness is conservative and may mark results potentially stale.
- Assessment severity values are `blocker`, `high`, `medium`, `low`, and
  `info`.
- Assessment source-state values are `direct`, `tool_reported`,
  `user_reported`, `inferred`, `stale`, and `insufficient`.
- Core assessment sources are behavior proof, source control, CI status,
  dependency scans, security scans, mutation results, benchmarks, bug reports,
  feedback, and observation outcomes. Additional sources are adapter-provided.
- Default freshness windows are policy-controlled, with security and dependency
  scans treated as shorter-lived than behavior proof or accepted user feedback.
- Personal contexts may auto-route non-blocking findings to investigation.
  Organizational contexts require approval for product-direction changes,
  policy changes, credential changes, and new roadmap commitments.
- Normalized findings include source, source-state, severity, affected scope,
  affected behavior or resource where known, rationale, source references,
  freshness, redaction state, recommended route, and dedupe key.

## Rejected Alternatives

- **Assessment as PR gating.** Rejected because broad or expensive analysis
  such as mutation testing should influence planning without blocking every
  spec by default.
- **Assessment changing behavior acceptance.** Rejected because behavior
  acceptance is product intent and belongs to planning.
- **Behavior files storing assertion state.** Rejected because assertion is
  proof-derived and can become stale independently of product intent.
- **Every finding requiring a behavior link.** Rejected because some findings
  reveal missing behavior, architecture gaps, provider issues, or unknown
  context.
- **Automatic roadmap mutation from analysis.** Rejected because signals must
  be classified and routed before accepted work changes.
- **Provider-native findings as canonical assessment state.** Rejected because
  Tanren needs normalized severity, provenance, source-state, routing, and
  linkage across providers.
