---
schema: tanren.subsystem_architecture.v0
subsystem: observation
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Observation Architecture

## Purpose

This document defines Tanren's observation architecture. Observation is the
read-side understanding layer that turns canonical state and subsystem-produced
signals into scoped, explainable views of progress, quality, risk, health,
throughput, cost, and recent change.

Observation is not a dashboard-only subsystem. Dashboards are one interface for
observation read models. The subsystem also owns read-only reports, observer
digests, status summaries, trend views, bounded forecasts, changed-since
summaries, and provenance-aware status models.

Observation does not own the underlying facts it summarizes. Planning,
behavior proof, assessment, orchestration, runtime, provider integrations,
client integrations, operations, state, identity-policy, and configuration
produce the canonical events and read models. Observation composes those inputs
into durable snapshots and queryable views.

## Subsystem Boundary

The observation subsystem owns:

- project overview models;
- work pipeline views;
- roadmap progress summaries;
- blocked-work overviews;
- quality, health, and risk signal views;
- time-window comparisons;
- organization and account-level operational metrics;
- delivery and cycle-time metrics;
- DORA-style delivery metrics where derivable;
- cost, quota, and execution live-time summaries;
- bounded forecasts and forecast drivers;
- recently shipped outcome summaries;
- post-release health and feedback summaries;
- cross-project dependency risk summaries;
- read-only status report semantics;
- observer digest semantics;
- changed-since-last-report baselines;
- provenance, freshness, completeness, redaction, and bounds metadata for
  observation claims.

The observation subsystem does not own:

- canonical event storage;
- planning, roadmap, behavior, or architecture state;
- behavior-proof execution;
- assessment findings or classifications;
- spec, task, gate, review, walk, or merge lifecycle state;
- runtime placement, target health checks, or worker execution;
- provider API mechanics;
- client webhook delivery mechanics;
- identity, permission, or redaction policy;
- backup, restore, incident, or maintenance operation execution.

Observation consumes facts from those subsystems and exposes coherent,
permissioned, provenance-aware read models.

## Core Invariants

1. **Observation is read-side composition.** Observation does not create
   product truth, proof truth, assessment truth, orchestration truth, or
   runtime truth. It summarizes and compares state produced elsewhere.
2. **Metrics are concrete before interpretive.** Observation should prefer
   exact counts, durations, rates, states, bounds, and source-linked summaries
   over generic certainty scores.
3. **Provenance is visible.** Observation claims identify source, freshness,
   completeness, bounds, redaction, and whether a value is measured,
   estimated, inferred, or unavailable.
4. **Missing data is not healthy data.** Missing, stale, hidden, redacted, or
   unsupported inputs are exposed as such instead of being treated as zero,
   passing, or healthy.
5. **Reports and digests are durable snapshots.** Live dashboards are
   projections. Exported reports, sent digests, and changed-since baselines are
   recorded so later comparisons are stable.
6. **Observation is scoped.** Every view has an account, organization,
   project, group, milestone, time window, or resource scope.
7. **Redaction applies before summary.** Observation must not leak hidden data
   through totals, trends, drilldowns, comparison groups, errors, or exports.
8. **Cross-project observation is allowed.** Planning remains per-project, but
   metrics such as active specs, cycle time, cost, VM live time, DORA signals,
   queue pressure, and dependency risk can be observed across projects.
9. **People metrics are contextual.** Per-person views may exist for work
   coordination and improvement, but individual leaderboards and simplistic
   productivity scoring are rejected.
10. **Forecasts expose bounds and drivers.** Forecasts must show the measured
    inputs, assumptions, and uncertainty bounds behind the projection.

## Input Model

Observation consumes read models, event streams, and source links from other
subsystems.

Primary inputs include:

- planning goals, roadmap nodes, priorities, decisions, assumptions, and
  proposals;
- behavior catalog state and behavior-to-roadmap links;
- behavior-proof assertion state, proof runs, coverage interpretation, and
  mutation-quality signals;
- assessment classifications, findings, recommendations, severity, and routing;
- orchestration spec state, task state, gates, audits, adherence checks,
  demos, walks, reviews, merge state, blockers, and feedback loops;
- runtime worker, queue, lease, target, harness, gate, retry, cancellation,
  and recovery state;
- provider source-control, CI, issue-tracker, provider health, quota, and cost
  signals;
- client-reported external status, subscription, webhook, replay, and
  backpressure signals;
- operations backup, restore, incident, maintenance, cost, quota, pause/resume,
  and audit-export state;
- identity-policy redaction, visibility, actor, and scope rules;
- configuration that determines observation windows, shipped definitions,
  report settings, and digest preferences.

Observation inputs are not copied into a second canonical truth. Observation
read models retain references to source records and event positions where
possible.

## Observation Claim Model

An observation claim is any summarized statement exposed by a dashboard,
report, digest, API response, or export.

Observation claims include:

- claim identifier;
- scope;
- time window;
- source subsystem references;
- source event positions or projection checkpoints;
- generated-at timestamp;
- value or status;
- value kind;
- bounds where applicable;
- freshness state;
- completeness state;
- redaction state;
- unsupported or unavailable state;
- drilldown references where visible.

Value kinds include:

- `measured`: computed from complete visible source data;
- `bounded`: reported as a range because exact value is not useful or not
  knowable;
- `estimated`: derived from incomplete but sufficient visible data;
- `inferred`: derived from source signals with explicit assumptions;
- `unavailable`: source data is absent, hidden, stale, or unsupported;
- `redacted`: source data exists but is hidden from the actor.

Observation should not collapse these into a generic certainty score. If a
summary is uncertain, the view should show why: missing source, stale
projection, redaction, limited history, conflicting signals, or explicit
forecast bounds.

## Core Views

Observation owns a set of canonical read models that interfaces can render in
different ways.

### Project Overview

Project overview summarizes:

- product mission and current focus;
- roadmap progress;
- active specs;
- pending walks or reviews;
- blocked work;
- behavior-proof posture;
- current assessment findings;
- provider and runtime health affecting work;
- recent shipped outcomes;
- post-release follow-up state;
- attention needs.

Hidden or unavailable inputs are marked rather than treated as healthy.

### Work Pipeline

The work pipeline view summarizes where work sits across planning,
orchestration, review, merge, release, and follow-up.

Pipeline stages include:

- proposed;
- accepted roadmap node;
- shaped spec;
- running;
- checking;
- draft PR;
- candidate validation;
- ready for review;
- walk pending;
- code review pending;
- merge ready;
- shipped;
- post-release follow-up.

The exact states are derived from planning and orchestration read models.
Observation owns grouping, filtering, trend, and report semantics.

### Roadmap Progress

Roadmap progress connects product goals to behavior, roadmap nodes, specs,
walks, shipped outcomes, and follow-up work.

Observation reports:

- accepted behavior counts;
- behavior implementation assessment counts;
- behavior assertion counts;
- roadmap node status;
- shipped and superseded work;
- dependencies and blockers;
- product-goal progress where traceable.

Observation does not decide product priority or mutate roadmap state.

### Blocked Work

Blocked-work views show:

- blocked specs, tasks, roadmap nodes, provider actions, runtime targets, or
  approvals;
- blocking reason category;
- responsible subsystem;
- available next actions;
- age of block;
- affected goals or behaviors where known;
- recent change that introduced or changed the block.

Observation exposes why work is blocked. The owning subsystem resolves the
block.

### Quality, Health, And Risk

Quality and risk views summarize source signals from behavior proof,
assessment, orchestration, runtime, integrations, and operations.

Examples:

- asserted behavior count and ratio;
- behavior proof failures;
- mutation-quality concerns;
- audit and adherence failures;
- failing or stale CI/status checks;
- bug report categories;
- regression counts;
- security or dependency findings;
- provider health issues;
- runtime target failure rates;
- incident or maintenance state;
- post-release degraded signals.

Observation reports trend and current posture. Assessment owns findings and
classification. Behavior proof owns proof semantics.

### Operational Metrics

Organization and account-level metrics include:

- active specs;
- active workers;
- active execution targets;
- total VM or target live time;
- queue pressure;
- mean spec cycle time;
- mean task cycle time;
- mean time waiting for human response;
- mean time waiting for provider or CI response;
- runtime failure rates;
- provider quota and cost usage;
- webhook and subscription health;
- incident and maintenance windows.

These metrics are useful across projects even though product planning remains
per-project.

### DORA-Style Metrics

Where source-control, CI, release, and incident signals are available,
Observation may compute DORA-style metrics at project, organization, or account
scope:

- deployment frequency;
- lead time for changes;
- change failure rate;
- time to restore service.

DORA-style metrics are used for system learning and operational improvement,
not individual ranking.

### Recently Shipped Outcomes

Recently shipped views summarize completed work as product outcomes, not only
closed specs.

Outcomes distinguish:

- user-visible change;
- internal improvement;
- fix;
- risk reduction;
- follow-up work;
- post-release pending state.

Shipped definitions are configuration-owned. Observation applies configured
definitions when generating shipped summaries.

### Post-Release Health

Post-release views connect shipped work to later signals:

- health checks;
- support notes;
- bug reports;
- complaints;
- customer feedback;
- metrics;
- field observations;
- follow-up findings.

Signals distinguish healthy, degraded, missing, mixed, and needs-follow-up
states without inferring production health from absent data.

### Cross-Project Dependency Risk

Cross-project observation summarizes dependency and coordination risk when
explicit cross-project relationships exist.

Observation may show:

- dependency readiness;
- blocked dependents;
- shared provider or runtime pressure;
- cross-project release risk;
- shared incident or quota impact.

This does not turn planning into a cross-project roadmap system. It is a
visibility layer over explicit project relationships and shared operational
signals.

## Time Windows And Trends

Observation supports comparing time windows for any compatible view.

Time-window comparisons may show:

- counts;
- rates;
- durations;
- throughput;
- quality signal trends;
- risk signal trends;
- health signal trends;
- cost or quota trends;
- blocked time;
- waiting time;
- shipped outcomes.

Observation should preserve enough snapshot and source metadata to explain why
a trend changed. A trend is invalid or partial when source definitions changed,
visibility changed, data is missing, or the selected window is too small to be
meaningful.

## Forecasts

Forecasts are bounded projections based on measured history, current state, and
explicit drivers.

Forecasts may expose:

- estimated delivery window;
- throughput range;
- likely bottlenecks;
- blocker impact;
- provider or CI wait impact;
- runtime cost range;
- review or walk wait impact;
- major assumptions;
- source data limits.

Forecasts should avoid false precision. They report bounds and drivers rather
than generic certainty scores. If a forecast lacks enough source data, the
forecast should be unavailable or marked as partial with concrete missing
inputs.

## Reports

Read-only reports are durable observation snapshots.

Report records include:

- report identifier;
- scope;
- actor who generated or requested the report;
- report type;
- generated-at timestamp;
- source event positions and projection checkpoints;
- selected time window;
- included sections;
- redaction summary;
- freshness summary;
- exported format metadata;
- immutable snapshot content or reconstructable snapshot reference.

Reports can be exported through permitted interfaces. Publishing reports to
external systems requires configured client or provider integration permission.

## Digests

Observer digests are scheduled or subscribed observation snapshots.

Observation owns digest content semantics:

- scope;
- sections;
- time window;
- changed-since baseline;
- attention items;
- blocked work;
- shipped outcomes;
- risk and health changes;
- redaction and freshness summary.

Operations owns scheduling mechanics. Provider integrations or client
integrations own delivery mechanics depending on the delivery channel.

## Changed Since Last Report

Changed-since summaries compare the current observation state to a durable
prior report, digest, or explicit baseline.

They may include:

- newly accepted work;
- newly blocked or unblocked work;
- specs moved between pipeline stages;
- shipped outcomes;
- new findings;
- resolved findings;
- changed provider/runtime health;
- changed cost or quota posture;
- new post-release signals;
- changed forecast bounds.

The baseline must be explicit. Observation should not infer "last report" from
interface-local state.

## Per-Person And Team Views

Per-person and team views are allowed only as scoped, permissioned observation
views for coordination, fairness, capacity, and process improvement.

Permitted examples:

- work waiting on a specific person;
- review or walk requests assigned to a person;
- average time waiting for human response by scope;
- active attention load;
- handoff volume;
- approval queues;
- authored or accepted decisions where visible.

Rejected uses:

- individual productivity scores;
- DORA leaderboards;
- ranking developers by velocity;
- interpreting agent-produced output as direct human productivity;
- exposing private workloads through aggregate comparisons.

In a mature Tanren workflow, human work shifts toward product judgment,
research, review, walk participation, and system interaction. Observation
should reflect that reality rather than import simplistic engineering
performance dashboards.

## Redaction And Visibility

Observation enforces identity-policy at query and report generation time.

Redaction rules:

- hidden resources are omitted or redacted;
- secret values are never included;
- hidden provider, runtime, or client details are not leaked through counts;
- low-count aggregates may be suppressed where they could identify hidden
  information;
- report exports preserve the actor's visibility boundary;
- stale and redacted inputs are marked where safe;
- summaries should not imply completeness when visibility is partial.

Observation read models may need actor-aware generation or query-time
redaction. The implementation must choose the approach that preserves
performance without weakening policy.

## Events

Observation emits typed events for:

- report generated;
- digest generated;
- digest subscription created, updated, paused, resumed, or removed;
- observation snapshot created;
- changed-since baseline recorded;
- forecast generated;
- observation export generated;
- observation export accessed;
- observation metric definition changed;
- observation view configuration changed.

Routine live dashboard reads do not emit observation events unless an audit or
access policy requires it.

## Read Models

Required observation read models include:

- project overview;
- work pipeline;
- roadmap progress;
- blocked work;
- quality signal summary;
- risk signal summary;
- health signal summary;
- operational metrics;
- DORA-style metrics;
- cost and quota summary;
- recently shipped outcomes;
- post-release health;
- cross-project dependency risk;
- report list and detail;
- digest subscriptions;
- changed-since summary;
- forecast summary;
- per-person and team attention views where permitted.

Each read model must include scope, freshness, source position, and visibility
metadata where applicable.

## Tracing Initialization Contract

Every binary in `bin/*/src/main.rs` MUST call
`tanren_observability::init(env_filter)` as its first action, before any
other tracing emission, log line, or service-status `println!`.

`tanren_observability::init` centralizes the tracing-subscriber setup:
JSON-formatted output, env-filtered via the standard
`RUST_LOG`/`TANREN_LOG` envelope, and span/event metadata aligned with
the cross-binary correlation contract in this document.

CLI service-status messages (`Starting tanren-cli...`,
`Connected to API at ...`, etc.) route through `tracing::info!` to
**stderr**, not via `writeln!(io::stdout(), ...)`. This frees stdout for
the structured event identifiers the existing CLI output contract
relies on (script consumers grep machine-readable lines from stdout;
status chatter goes to stderr). The CLI output contract is unchanged;
only its delivery mechanism — `tracing` events to stderr instead of ad
hoc prints — is canonicalized.

### TUI exception: file-sink only

Binaries that own the terminal — currently only `tanren-tui` — MUST call
`tanren_observability::init_to_file(default_filter, "tanren-tui")`
instead of `init(...)`. The TUI calls `enable_raw_mode()` and
`EnterAlternateScreen` on stdout
(`crates/tanren-tui-app/src/lib.rs::setup_terminal`); a stdout- or
stderr-bound subscriber would render `INFO …` lines on top of the
ratatui frame and corrupt the rendered UI.

`init_to_file` installs a non-blocking, daily-rolling file appender:

| Resolution order | Path |
|---|---|
| `TANREN_TUI_LOG_FILE` set, non-empty | the value verbatim (parent dir auto-created); single file, no rolling. |
| else `XDG_STATE_HOME` set, non-empty | `$XDG_STATE_HOME/tanren/logs/tanren-tui.log.YYYY-MM-DD` |
| else `HOME` set, non-empty | `$HOME/.local/state/tanren/logs/tanren-tui.log.YYYY-MM-DD` |
| else | `./tanren/logs/tanren-tui.log.YYYY-MM-DD` |

The function returns a `tracing_appender::non_blocking::WorkerGuard`
which the binary's `main` MUST bind to a named local for the duration of
the process (e.g. `let _log_guard = tanren_observability::init_to_file(…)?;`)
— dropping the guard flushes the channel and silently stops emitting
log records.

This rule is symmetric with the stdout discipline above: the CLI keeps
stdout clean for its structured output; the TUI keeps stdout clean for
its rendered frame. Neither writes log records to a stream another
contract owns.

### Mechanical enforcement

`xtask check-tracing-init` AST-walks every `bin/*/src/main.rs` and
enforces:

1. Every binary calls `tanren_observability::init(...)` or
   `tanren_observability::init_to_file(...)` as its first action.
2. `bin/tanren-tui/src/main.rs` MUST call `init_to_file(...)` and MUST
   NOT call the plain stdout-bound `init(...)`.

A regression that reintroduces the stdout subscriber for the TUI fails
`just check` before it can land.

## Accepted Decisions

- Observation is a read-side composition subsystem, not canonical product or
  execution state.
- Dashboards are projections. Reports, sent digests, and changed-since
  baselines are durable snapshots.
- Observation reports concrete metrics, facts, ranges, and provenance instead
  of generic certainty scores.
- Missing, stale, hidden, redacted, and unsupported inputs are visible as such.
- Forecasts expose bounds, drivers, and missing inputs rather than false
  precision.
- Cross-project observation is supported for metrics and risks that generalize
  cleanly across projects.
- DORA-style metrics are organization and project learning tools, not
  individual leaderboards.
- Per-person views are permissioned and contextual, focused on coordination and
  improvement rather than ranking.
- Digest semantics belong to observation. Scheduling belongs to operations.
  Delivery belongs to provider or client integrations depending on channel.
- Default observation windows are 7, 30, and 90 days, with custom windows
  allowed by policy.
- Tanren derives deployment frequency, change lead time, change failure rate,
  and restore time only when source-control, merge, release, incident, and
  recovery source signals support them.
- Organizations may configure metric definitions, shipped-work boundaries,
  incident boundaries, working-time calendars, and visibility filters.
- Aggregate views suppress low counts when fewer than 5 visible records or
  fewer than 3 visible actors support the metric.
- First report export formats are Markdown, JSON, and PDF.
- Default observer digests include work pipeline, blocked work, shipped
  outcomes, behavior proof status, quality and risk, provider health, budget
  warnings, and changed-since-last-report.
- Reports store rendered snapshot content plus source event positions and
  source references so the exact report remains auditable after projections
  change.

## Rejected Alternatives

- **Observation as interface-local dashboards.** Rejected because reports,
  digests, subscriptions, exports, and APIs need shared read models with
  freshness and redaction semantics.
- **Generic certainty scoring as the primary observation model.** Rejected
  because concrete metrics, provenance, freshness, completeness, redaction, and
  bounds are more useful and less misleading.
- **Missing data as healthy data.** Rejected because unavailable signals should
  not imply absence of risk.
- **Forecasts with precise dates but hidden assumptions.** Rejected because
  delivery projections must expose bounds, drivers, and missing inputs.
- **DORA leaderboards or individual productivity rankings.** Rejected because
  they create harmful incentives and do not match Tanren's agentic delivery
  model.
- **Cross-project planning through observation.** Rejected because observation
  can summarize cross-project risk and metrics without becoming the planning
  authority for multiple projects.
- **Reports regenerated from current state only.** Rejected because
  changed-since comparisons and external sharing require stable historical
  snapshots.
