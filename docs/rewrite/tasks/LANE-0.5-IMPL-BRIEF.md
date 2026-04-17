# Lane 0.5 — Implementation Brief (Rust + Self-Hosting)

> **You are continuing Lane 0.5 execution.** The specification and
> command-source refactor landed on `lane-0.5` in commit `668253c`.
> Your job is to deliver the Rust implementation and the self-hosting
> wiring that consume it, then push a single additional commit on the
> same branch with a clean `just ci` gate.

## 0. Read these first (in order)

These are non-optional. Do not skip them; the design rationale
influences every decision.

1. **[LANE-0.5-DESIGN-NOTES.md](LANE-0.5-DESIGN-NOTES.md)** — 20
   core design decisions with rationale, alternatives, and
   Phase 1+ implications. This is the authoritative intent record;
   if any other doc conflicts with it, *this* doc wins.
2. **[LANE-0.5-BRIEF.md](LANE-0.5-BRIEF.md)** — the 15 execution
   non-negotiables. Memorize them.
3. **[LANE-0.5-AUDIT.md](LANE-0.5-AUDIT.md)** — the mechanical
   grep checklist + typed-domain / tool-surface / installer audit
   dimensions. Your work must pass this audit.
4. **[LANE-0.5-METHODOLOGY.md](LANE-0.5-METHODOLOGY.md)** — original
   planning contract (with status note about superseded framing).
5. **Architecture specs** (all authoritative; treat as spec):
   - [orchestration-flow.md](../../architecture/orchestration-flow.md) — state machine, mermaid diagrams, escalation ladder, monotonicity, cross-spec concerns.
   - [agent-tool-surface.md](../../architecture/agent-tool-surface.md) — tool catalog, transports (rmcp + CLI), capability scopes, phase-events.jsonl contract, versioning.
   - [evidence-schemas.md](../../architecture/evidence-schemas.md) — typed frontmatter for spec/plan/demo/audit/signposts/investigation-report.
   - [audit-rubric.md](../../architecture/audit-rubric.md) — 13 built-in pillars, 1–10 scoring, finding-linkage invariants, extensibility.
   - [adherence.md](../../architecture/adherence.md) — adherence vs audit vs triage-audits; relevant-standard filtering.
   - [install-targets.md](../../architecture/install-targets.md) — format drivers (Claude Code, Codex Skills dir-per-command, OpenCode template-field, standards-baseline) + MCP config writers.
6. **Methodology docs**:
   - [METHODOLOGY_BOUNDARY.md](../METHODOLOGY_BOUNDARY.md) — operational ownership table, command-level split, manual self-hosting sequence.
   - [../../methodology/system.md](../../methodology/system.md) — command table, ownership boundary, agent tool surface overview.
   - [../../methodology/commands-install.md](../../methodology/commands-install.md) — install flow and config reference.
   - [../../architecture/phase-taxonomy.md](../../architecture/phase-taxonomy.md) — phase classification, verification-hook resolution chain, guard model.
7. **Rewrite canon** (these were aligned in 668253c; they are the
   shape the implementation must match):
   - [HLD.md](../HLD.md) §6 — methodology subsystem
   - [DESIGN_PRINCIPLES.md](../DESIGN_PRINCIPLES.md) principles 11, 12, 13
   - [ROADMAP.md](../ROADMAP.md) Phase 0 exit criteria
   - [CRATE_GUIDE.md](../CRATE_GUIDE.md) linking rule §7
   - [CLAUDE.md](../../../CLAUDE.md) — Rust conventions and quality rules (**every implementation agent must read this**)
8. **Command sources you will render**: [commands/](../../../commands/) — 11 under `spec/`, 6 under `project/`, plus `README.md`. Rendered artifacts must be semantically identical across Claude Code / Codex Skills / OpenCode targets.

## 1. What's already done (don't redo)

- All architecture specs, canon alignment, methodology docs, lane
  docs, and design notes.
- `commands/spec/` and `commands/project/` rewritten to the uniform
  templated tool-driven skeleton. No `gh`/`git`/`make`/`.agent-status`
  residue. All 17 commands declare `declared_variables`,
  `declared_tools`, `required_capabilities`, `produces_evidence` in
  frontmatter.
- `just ci` is green on `lane-0.5`.

## 2. What you are delivering

Two parts, one commit, one push.

### Part A — Rust implementation

Six crates touched. Follow existing workspace conventions:
edition 2024, `thiserror` in libraries, `anyhow` only in binaries,
no `unwrap`/`panic`/`todo`/`unimplemented`, no `println!`/`eprintln!`/
`dbg!` (use `tracing`), no inline `#[allow]`/`#[expect]`, ≤ 500
lines/file, ≤ 100 lines/function, secrets via `secrecy::Secret<T>`,
deps pinned in `[workspace.dependencies]`.

#### A.1 `tanren-domain::methodology`

[agent-tool-surface.md §8](../../architecture/agent-tool-surface.md#8-standard-vs-domain-types)
and [orchestration-flow.md §2](../../architecture/orchestration-flow.md#2-task-lifecycle)
are your spec.

New module tree under
[crates/tanren-domain/src/methodology/](../../../crates/tanren-domain/src/methodology/):

- `task.rs` — `Task`, `TaskId` (uuid-v7 newtype), `TaskStatus`
  (`Pending | InProgress | Implemented | Complete | Abandoned` with
  per-guard flags tracked on `Implemented+`), `TaskOrigin` (full
  enum from orchestration-flow §2.3).
- `finding.rs` — `Finding`, `FindingId`, `FindingSeverity`
  (`FixNow | Defer | Note | Question`), `FindingSource`,
  `StandardRef`.
- `pillar.rs` — `Pillar` (id, name, task_description,
  spec_description, target_score, passing_score, applicable_at);
  the 13 built-in defaults.
- `rubric.rs` — `RubricScore`; scoring invariants enforced in
  constructor (`score < target` requires findings;
  `score < passing` requires `fix_now` findings).
- `standard.rs` — `Standard` (with `applies_to`,
  `applies_to_languages`, `applies_to_domains`, `importance`).
- `phase_outcome.rs` — `PhaseOutcome` (`Complete | Blocked | Error`),
  typed reason enums.
- `capability.rs` — `ToolCapability`, per-phase capability sets
  (matches [agent-tool-surface.md §4](../../architecture/agent-tool-surface.md#4-per-phase-capability-scopes)).
- `evidence/` — `SpecFrontmatter`, `PlanFrontmatter`,
  `DemoFrontmatter`, `AuditFrontmatter`, `SignpostsFrontmatter`,
  `InvestigationReport`. Each with `parse_from_markdown` and
  `render_to_markdown`. See [evidence-schemas.md](../../architecture/evidence-schemas.md) §2 for exact shapes.
- `events.rs` — extend the existing `DomainEvent` with:
  `TaskCreated`, `TaskStarted`, `TaskImplemented`, `TaskGateChecked`,
  `TaskAudited`, `TaskAdherent`, `TaskXChecked` (extensible guard
  variant), `TaskCompleted`, `TaskAbandoned`, `TaskRevised`,
  `FindingAdded`, `RubricScoreRecorded`, `AdherenceFindingAdded`,
  `PhaseOutcomeReported`, `UnauthorizedArtifactEdit`,
  `EvidenceSchemaError`, `IssueCreated`, `SpecDefined`.
- `mod.rs` — public surface.

Test coverage (mandatory):
- `proptest` for state-machine monotonicity: `Complete` is terminal;
  every permutation of guard arrivals converges to the same
  `Implemented+guards` state; `TaskCompleted` fires iff required
  guards are all present.
- `insta` snapshots for canonical JSON of every enum variant and
  frontmatter schema (round-trip).
- Contract tests for `parse_from_markdown(render_to_markdown(x)) == x`.
- Rubric invariant tests: illegal scores rejected with typed errors.

#### A.2 `tanren-contract::methodology`

JSON Schema surface for every tool in the catalog
([agent-tool-surface.md §3](../../architecture/agent-tool-surface.md#3-tool-catalog-by-capability-group)).
Derived from Rust types via `schemars`. Stable `tanren.methodology.v1`
namespace. Backward-compatible additions = minor bump; breaking
changes = major bump.

New module [crates/tanren-contract/src/methodology/](../../../crates/tanren-contract/src/methodology/).

Per `CRATE_GUIDE.md`: contract crate is serialization/schema only,
no business logic.

#### A.3 `tanren-store::methodology`

Extend the existing `DomainEvent`-keyed event log — **no new
tables**. Add SeaORM migrations for the new event variants (sqlite
+ postgres dialect coverage; both backends must work).

New projections:
- `tasks_for_spec(spec_id) → Vec<Task>`
- `findings_for_task(task_id) → Vec<Finding>`
- `findings_for_spec(spec_id) → Vec<Finding>`
- `adherence_findings_for_spec(spec_id) → Vec<Finding>`
- `signposts_for_spec(spec_id) → Vec<Signpost>`
- `rubric_for_spec(spec_id) → Vec<RubricScore>`
- `replay(spec_folder) → Result<()>` — ingest a `phase-events.jsonl`
  file into the store.

Integration tests (sqlite + postgres): monotonicity guard,
event replay, projection correctness, guard-set composition under
out-of-order event arrival.

#### A.4 `tanren-app-services::methodology`

This is the bulk of the Rust work. Per `CRATE_GUIDE.md`:
methodology resolution is an app-services concern.

New module tree under
[crates/tanren-app-services/src/methodology/](../../../crates/tanren-app-services/src/methodology/):

- `service.rs` — the orchestrator-owned API mirroring the tool
  catalog 1:1. Each method: validate inputs (typed `ToolError` on
  failure per agent-tool-surface §5), emit event(s), update
  projections, atomically append to `phase-events.jsonl`.
- `ingest.rs` — strict JSONL parse for
  `tanren ingest-phase-events`. Malformed line = typed error with
  line number + original content.
- `enforcement.rs` — three-layer artifact enforcement: pre-session
  `chmod 0444`, postflight diff + auto-revert, emits
  `UnauthorizedArtifactEdit`. Applies to `plan.md`, `progress.json`,
  any generated index.
- `evidence.rs` — frontmatter render-from-events + schema validation
  in postflight for agent-authored narrative files.
- `rubric.rs` — pillar resolution (built-ins + `tanren/rubric.yml`
  overrides), scoring-invariant enforcement, pillar applicability
  filter for task vs spec scope.
- `adherence.rs` — relevant-standard filter per
  [adherence.md §4.1](../../architecture/adherence.md#41-algorithm);
  adherence-finding recording with critical-cannot-defer rule.
- `renderer.rs` — template variable resolution + substitution;
  canonical `RenderedCommand` IR; hard errors on unknown /
  declared-but-unused / referenced-but-undeclared variables.
- `source.rs` — read `commands/spec/` + `commands/project/`;
  parse frontmatter.
- `installer.rs` — `InstallPlan` / `InstallOutcome`; atomic
  tempfile+rename; dry-run + strict modes; per-merge-policy
  application.
- `formats/` — one driver per target:
  - `claude_code.rs` — `.claude/commands/<name>.md`, YAML fm +
    md body
  - `codex_skills.rs` — `.codex/skills/<name>/SKILL.md` (dir per
    command)
  - `opencode.rs` — `.opencode/commands/<name>.md` (prompt body in
    `template` frontmatter field)
  - `standards_baseline.rs` — per-category standards files,
    `preserve_existing`
  - `claude_mcp_json.rs`, `codex_config_toml.rs`, `opencode_json.rs`
    — MCP config writers with `preserve_other_keys` semantics
- `capabilities.rs` — per-phase capability set resolution
  (consulted by both MCP and CLI transports).
- `errors.rs` — `MethodologyError` umbrella (`thiserror`) + typed
  `ToolError` variants per agent-tool-surface §5.

Tests:
- Service method per tool: valid input → expected event + projection;
  invalid input → typed `ToolError` with correct `field_path` /
  `expected` / `actual` / `remediation`.
- Capability scope: out-of-scope tool calls rejected.
- Multi-target parity: canonicalized-form hash equal across the
  three agent formats.
- Standards preserve: hand-edited standard survives reinstall.
- MCP config preserve_other_keys: existing keys untouched.
- Enforcement: write to read-only artifact → auto-revert +
  `UnauthorizedArtifactEdit` emitted.
- Evidence schema: malformed frontmatter → typed error; round-trip
  stable.
- Rubric: low score without linked findings → rejected.

#### A.5 `tanren-cli` subcommands

New subcommands under [bin/tanren-cli/src/commands/](../../../bin/tanren-cli/src/commands/):

- `install.rs` — `tanren install [--profile --config --source
  --target --dry-run --strict]`. Exit codes: `0` ok, `1`
  config/render error, `2` write error, `3` dry-run drift,
  `4` validation error.
- `task.rs` — `tanren task {create|start|complete|revise|abandon|list}`.
- `finding.rs` — `tanren finding add`.
- `rubric.rs` — `tanren rubric record`.
- `compliance.rs` — `tanren compliance record`.
- `spec.rs`, `demo.rs`, `signpost.rs` — mirrors of the frontmatter
  tools in agent-tool-surface §3.3–3.5.
- `phase.rs` — `tanren phase {outcome|escalate|reply}`.
- `issue.rs` — `tanren issue create`.
- `standard.rs` — `tanren standard list`.
- `adherence.rs` — `tanren adherence add-finding`.
- `ingest.rs` — `tanren ingest-phase-events <spec_folder> [--follow]`.
- `replay.rs` — `tanren replay <spec_folder>`.

All use `clap` derive. Exit codes typed. `tracing` to stderr.

Tests: `assert_cmd` integration per subcommand + golden-directory
compares via `insta` for install output.

#### A.6 `tanren-mcp` binary

New or significantly expanded [bin/tanren-mcp/](../../../bin/tanren-mcp/)
using `rmcp` (`modelcontextprotocol/rust-sdk`, features `server`,
`transport-io`, `macros`, tokio runtime).

- Register each tool in the catalog via `#[tool_router]` + `#[tool(…)]`
  attribute macros. Schemas derived from the contract types.
- stdio transport only (Lane 0.5 scope).
- `TANREN_PHASE_CAPABILITIES` env var (supplied by the orchestrator
  at dispatch) drives capability-scope enforcement; out-of-scope
  calls return `CapabilityDenied`.
- `tracing_subscriber::fmt().with_writer(std::io::stderr).init();` —
  **never** write to stdout; stdio framing will corrupt.
- Handshake version negotiation honored; pin the `rmcp` major at
  implementation time via `cargo search rmcp`. Before adding the
  dep, **verify the license against `deny.toml`'s allowlist**
  (MIT/Apache-2.0 expected but not guaranteed).
- Backend = the same `methodology::service` methods; both transports
  produce identical events.

Tests: spawn `tanren-mcp` in a fixture; drive a test MCP client
through each tool with valid + invalid input; assert event trail
matches CLI transport for identical inputs.

### Part B — Self-hosting wiring (tanren-repo only)

#### B.1 `tanren.yml`

Root of repo. Add the `methodology:` section per
[install-targets.md §5](../../architecture/install-targets.md#5-config)
plus `variables:` per
[install-targets.md §4.1](../../architecture/install-targets.md#41-taxonomy):

```yaml
methodology:
  task_complete_requires: [gate_checked, audited, adherent]
  source:
    path: commands
  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive
    - path: .codex/skills
      format: codex-skills
      binding: mcp
      merge_policy: destructive
    - path: .opencode/commands
      format: opencode
      binding: mcp
      merge_policy: destructive
    - path: tanren/standards
      format: standards-baseline
      binding: none
      merge_policy: preserve_existing
  mcp:
    transport: stdio
    enabled: true
    also_write_configs:
      - path: .mcp.json
        format: claude-mcp-json
        merge_policy: preserve_other_keys
      - path: .codex/config.toml
        format: codex-config-toml
        merge_policy: preserve_other_keys
      - path: opencode.json
        format: opencode-json
        merge_policy: preserve_other_keys
  variables:
    task_verification_hook: "just check"
    spec_verification_hook: "just ci"
    issue_provider: GitHub
    project_language: rust
```

#### B.2 `tanren/rubric.yml`

Create the file with the 13 built-in pillars per
[audit-rubric.md §3](../../architecture/audit-rubric.md#3-built-in-pillars-13-defaults).
Entries match the defaults verbatim so this repo exercises the full
taxonomy.

#### B.3 `justfile`

Add **tanren-repo-specific** recipes (document as such — these are
dogfooding, not prescribed to downstream adopters):

```justfile
install-commands:
    cargo run -p tanren-cli -- install

install-commands-check:
    cargo run -p tanren-cli -- install --strict --dry-run
```

Extend `just ci` to run `install-commands-check`. Drift in the
rendered directories becomes a CI failure.

#### B.4 Rendered artifacts

Run `just install-commands` to populate:
- `.claude/commands/*.md`
- `.codex/skills/*/SKILL.md`
- `.opencode/commands/*.md`
- `.mcp.json` (with `tanren` MCP server registration)
- `.codex/config.toml` (with `[mcp_servers.tanren]`)
- `opencode.json` (with `mcp.tanren`)

Commit all of them. `.gitignore` must not exclude any.

#### B.5 Self-hosting proof

After the install: open a rendered `.claude/commands/do-task.md`.
Confirm:
- Zero residual `{{…}}` tokens.
- Prose reads as directive.
- Tool-call block references the correct binding (MCP).
- `{{READONLY_ARTIFACT_BANNER}}` renders to the three-layer warning.

## 3. Quality bar — 10 pillars

Every piece of work must aim for 10/10 on each pillar. 7/10 per
pillar is the minimum passing bar (aligns with the rubric the
implementation itself enforces). Where the audit rubric applies to
agents, these apply to the implementation:

1. **Completeness** — every tool in the catalog implemented; every
   command refactor already shipped renders correctly for every
   target; no hidden TODOs; no partial state machines.
2. **Performance** — append-only event log; pure renderer; O(events)
   projections; no hot paths touched; no quadratic scans; MCP stdio
   uses zero-copy where the transport allows.
3. **Scalability** — typed state scales from one spec to thousands;
   store works on both sqlite and postgres; pluggable pillars,
   guards, and format drivers; replay scales linearly.
4. **Strictness** — state machine guards return typed errors; no
   stringly-typed state; tool inputs schema-validated at boundary;
   `thiserror` everywhere; `schemars` for JSON schemas; proptest
   covers state machine; insta covers round-trips.
5. **Security** — no new network surface; MCP stdio is local;
   secrets flow only via `secrecy::Secret`; license of `rmcp`
   verified against `deny.toml`; tool capability scopes enforced;
   postflight reverts unauthorized writes.
6. **Stability** — Python left alone; new Rust surface fully
   tested; deterministic install; drift gate protects tanren's own
   rendered artifacts; all retries are fresh sessions.
7. **Maintainability** — small single-purpose modules; one tool
   catalog; one variable taxonomy; one rubric model; one install
   model; files ≤ 500 lines; functions ≤ 100 lines.
8. **Extensibility** — add a pillar = one `rubric.yml` entry; add a
   guard = new event variant + config line; add a tool = one
   catalog entry + one service method; add an install format =
   new trait impl; add an issue provider = new adapter.
9. **Elegance** — tools for verbs, types for nouns, events for
   history; no boilerplate-for-boilerplate's-sake; render logic is
   pure; I/O confined to source + install + ingest modules.
10. **Style** — 2024 edition, `thiserror` in libraries, `anyhow`
    only in bins, derive-based clap, serde + serde_yaml + toml,
    tracing with stderr writer for MCP, conventional commits, zero
    `#[allow]`/`#[expect]`, workspace-level lints honored,
    `cargo-machete` clean, `cargo deny` clean.

The **audit rubric** in
[audit-rubric.md](../../architecture/audit-rubric.md)
specifies the same pillars formally and is what will be applied
when the lane is audited.

## 4. Non-negotiables (from [LANE-0.5-BRIEF.md](LANE-0.5-BRIEF.md))

Repeating for foreground:

1. Task state is monotonic; `Complete` is terminal. Property-test.
2. No `.agent-status` file anywhere.
3. No markdown checkbox parsing as source of truth. `plan.md` is
   generated.
4. Agents never write orchestrator-owned artifacts. Three-layer
   enforcement.
5. Unknown / declared-but-unused / referenced-but-undeclared
   template variables = hard errors.
6. `escalate_to_blocker` callable only from `investigate`.
7. Fresh session on every retry.
8. Install is deterministic and idempotent; `--strict --dry-run`
   fails on drift with exit 3.
9. Multi-target parity by canonicalized-hash equality.
10. Commands install destructively; standards preserve existing;
    MCP configs preserve other keys.
11. Self-hosting drift gate is tanren-repo-specific; don't
    prescribe downstream CI.
12. Python untouched. No compatibility work.
13. Rubric scoring invariants enforced at `record_rubric_score`
    call time.
14. MCP server never writes stdout. `tracing` to stderr only.
15. `rmcp` license verified against `deny.toml` before dep added.

## 5. Verification — Wave 9 checklist

Sequential; do not skip any step.

1. **Static sweep — zero hits:**
   ```
   rg -n '^\s*(gh|git|make|just ci|cargo|docker)\b' commands/
   rg -n '\.agent-status' commands/
   rg -n 'find the next|select the next|choose a gate|create the issue' commands/
   rg -n 'edit plan\.md|update plan\.md|check off' commands/
   rg -n 'TODO|FIXME' crates/tanren-domain/src/methodology/
   ```
2. **Canon cross-check** — skim the 13 docs in §0 for drift; fix if
   found.
3. **Build** — `cargo build --workspace` green.
4. **Test** — `cargo nextest run` green (including new property +
   insta + contract + integration tests).
5. **Installer smoke:**
   ```
   cargo run -p tanren-cli -- install --dry-run
   cargo run -p tanren-cli -- install
   cargo run -p tanren-cli -- install   # second run = no-op
   cargo run -p tanren-cli -- install --strict --dry-run   # exit 0 (no drift)
   # Hand-edit .claude/commands/do-task.md, then:
   cargo run -p tanren-cli -- install --strict --dry-run   # exit 3 with diff
   ```
6. **Multi-target parity** — integration test in
   `crates/tanren-app-services/tests/install_parity.rs`.
7. **Standards preserve** — hand-edit
   `tanren/standards/<cat>/<s>.md`; reinstall; no overwrite.
8. **MCP smoke** — launch `tanren-mcp` in stdio mode with a test
   client fixture; round-trip every tool with valid + invalid input;
   errors are typed.
9. **Capability enforcement** — out-of-scope tool call → typed
   `CapabilityDenied`.
10. **Guard independence** — events arriving out of order converge
    correctly.
11. **Rubric invariants** — `record_rubric_score(pillar=security,
    score=3)` without linked `fix_now` findings → rejected.
12. **Adherence smoke** — violate a standard; `adhere-task` emits
    a finding; task's `Adherent` guard stays unsatisfied until
    resolved.
13. **Evidence schema** — break frontmatter via the orchestrator
    write path; service refuses with typed error.
14. **Enforcement smoke** — agent writes to `plan.md`; postflight
    reverts; `UnauthorizedArtifactEdit` emitted.
15. **`just ci` green** — including the new
    `install-commands-check` recipe.
16. **Stage + commit + pre-commit + push:**
    ```
    git add crates/ bin/ Cargo.toml Cargo.lock commands/ \
            .claude/commands/ .codex/skills/ .opencode/commands/ \
            .mcp.json .codex/config.toml opencode.json \
            tanren.yml tanren/rubric.yml justfile \
            docs/
    git status --short   # verify no strays (python, unrelated CI, etc.)
    git commit -m "$(cat <<'EOF'
    feat(methodology): lane 0.5 Rust implementation and self-hosting

    <wave-by-wave summary; variable taxonomy; tool catalog; pillar
    coverage; self-hosting proof; refs LANE-0.5-IMPL-BRIEF.md and
    LANE-0.5-DESIGN-NOTES.md>

    Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
    EOF
    )"
    # pre-commit runs (lefthook: taplo, fmt, clippy)
    git push origin lane-0.5
    ```
17. **Final check** — `git status` clean; HEAD advanced; remote
    CI green on `lane-0.5`.

## 6. Done when

Every item true at final audit:

1. All mechanical sweeps return zero hits.
2. All crate builds green; `cargo nextest run` green including new
   property, insta, contract, and integration tests.
3. `just ci` green across the full workspace including
   `install-commands-check`.
4. `tanren install --dry-run` + `tanren install` + re-run produce
   deterministic, idempotent output. `--strict --dry-run` fails on
   drift (exit 3).
5. Rendered `.claude/commands/`, `.codex/skills/`,
   `.opencode/commands/` committed in the tanren repo as
   self-hosting proof. All three contain semantically identical
   content.
6. `.mcp.json`, `.codex/config.toml`, `opencode.json` each register
   `tanren-mcp` correctly and preserve other keys.
7. `tanren-mcp` launches, registers the full tool catalog, enforces
   capabilities, round-trips events identically to the CLI.
8. Typed state machine proven monotonic by proptest; guard
   composition proven parallel-safe.
9. Rubric invariants enforced at tool call; adherence critical rule
   enforced; three-layer artifact enforcement reverts unauthorized
   edits.
10. `git status` clean; `lane-0.5` pushed; remote CI green.

## 7. Scope boundary — what is NOT in Lane 0.5

Per [LANE-0.5-DESIGN-NOTES.md §7](LANE-0.5-DESIGN-NOTES.md):

- Harness / environment-lease implementation (Phase 1).
- Planner-native orchestration (Phase 2).
- Final enterprise governance (Phase 3).
- Linear issue adapter (GitHub only this lane; Linear is a
  follow-up per `user_role` memory).
- Python compatibility work.
- Downstream-consumer CI recipe prescription.
- Non-stdio MCP transports.
- On-disk secret handling (MCP is local; secrets stay behind
  `secrecy::Secret<T>`).

## 8. Auxiliary

- Agent-scoped execution plan (historical, may be stale; the
  authoritative sources are the in-repo docs referenced in §0):
  `~/.claude/plans/read-the-instructions-at-sunny-starlight.md`.
- Prior commit landing the spec + command refactor: `668253c` on
  `lane-0.5`.

Build carefully. Every typed surface you create becomes load-
bearing for Phase 1+.
