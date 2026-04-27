# Audit Rubric

Authoritative spec for the opinionated audit rubric applied by
`audit-task` and `audit-spec`. Defines pillars, scoring rules,
finding-linkage invariants, pass criteria, and extensibility.

Companion docs:
[orchestration-flow.md](orchestration-flow.md),
[adherence.md](adherence.md),
[agent-tool-surface.md](agent-tool-surface.md).

---

## 1. Why a rubric separate from adherence

The rubric is **opinionated quality judgment** — "is this good
work?" It uses a 1–10 scored scale per pillar, demands human-readable
rationale, and requires linked findings for any sub-target score.

[Adherence](adherence.md) is **mechanical rule compliance** — "does
this follow the rules the team codified?" It is pass/fail per
standard, applied only to relevant standards, and does not produce
scores.

Conflating the two would either flatten nuanced judgment into
pass/fail or stretch binary compliance into false rubric scores.

---

## 2. Scoring

### 2.1 Scale

- Range: **1–10**.
- Target: **10** (per-pillar configurable).
- Passing: **7** (per-pillar configurable).

### 2.2 Scoring rules (enforced at tool call)

`record_rubric_score(pillar, score, rationale, supporting_finding_ids)`
validates:

| Condition | Requirement |
|---|---|
| `score = 10` | `supporting_finding_ids` may be empty. |
| `pillar.passing ≤ score < pillar.target` | At least one linked finding; severity `fix_now` or `defer` at auditor's discretion. |
| `score < pillar.passing` | At least one linked finding with severity `fix_now`. |

Violations → `ToolError::RubricInvariantViolated { pillar, score,
reason }`. Agent must either raise the score or add findings.

### 2.3 Pass criteria

A rubric pass requires all four of:
1. Every applicable pillar scores ≥ its `passing_score`.
2. Every non-negotiable compliance check returns `pass`.
3. Demo signal is `pass` (spec-level only).
4. Zero unaddressed `fix_now` findings remain.

Tasks with pillar scores in `[passing, target)` may defer linked
findings to backlog via `create_issue`; the spec still passes.

---

## 3. Built-in pillars (13 defaults)

Each pillar has `id`, `name`, `task_description`, `spec_description`,
`target_score` (default 10), `passing_score` (default 7), and
`applicable_at` (subset of `[task, spec]`).

### 3.1 Completeness
- **Task:** acceptance criteria fully met; downstream prerequisite
  tasks ready to start; no silent deferrals.
- **Spec:** all tasks `Complete`; zero silent deferrals; all
  acceptance criteria verified by demo; no orphaned Phase 1+ hand-
  offs.

### 3.2 Performance
- **Task:** no gratuitous inefficiency in the implemented surface;
  benchmarks where SLAs apply.
- **Spec:** end-to-end benchmarks clean; no regressions; scaling
  characteristics documented where relevant.

### 3.3 Scalability
- **Task:** scales from N=1 to large N; no hard-coded constants that
  break at scale; unbounded iteration protected.
- **Spec:** spec's full surface scales per the contract; pagination,
  cursoring, bounded loads accounted for.

### 3.4 Strictness (compile-time verification)
- **Task:** invariants encoded in types, not runtime checks; no
  stringly-typed state; `unwrap`/`panic` absent in library code.
- **Spec:** spec surface has no runtime-checkable invariants that
  could have been moved to types.

### 3.5 Security
- **Task:** no secrets in logs; inputs validated at boundaries;
  authz enforced for new surface; secrets wrapped in
  `secrecy::Secret`.
- **Spec:** threat model reviewed for the full surface; new attack
  surfaces documented; audit trail present for sensitive ops.

### 3.6 Stability
- **Task:** no panics; tests deterministic; races absent; retries
  and backoff used correctly.
- **Spec:** demo suite stable; CI stable; no Heisenbugs introduced.

### 3.7 Maintainability
- **Task:** module/function boundaries sensible; names precise; dead
  code absent.
- **Spec:** spec's outputs leave the codebase more navigable than
  it started.

### 3.8 Extensibility
- **Task:** pluggable where variation is likely; no premature
  abstraction.
- **Spec:** clean extension points for dependent future specs;
  evolvable contracts at public surfaces.

### 3.9 Elegance
- **Task:** simplest solution that solves the real problem; no
  boilerplate-for-boilerplate's-sake.
- **Spec:** spec-level architecture is crisp; no vestigial
  scaffolding.

### 3.10 Style
- **Task:** matches existing code; 2026 best practices; no obsolete
  patterns kept for their own sake.
- **Spec:** whole-surface style consistency; new conventions (if
  any) documented.

### 3.11 Relevance
- **Task:** all changes related to the task at hand; no unrelated
  drive-by edits.
- **Spec:** no scope creep beyond the shaped spec; scope excursions
  either closed or deferred with issues.

### 3.12 Modularity
- **Task:** boundaries honor the dependency DAG; no cross-cutting
  leaks; one concern per module.
- **Spec:** spec-scope respects crate/module boundaries; new
  abstractions live in the right crate.

### 3.13 Documentation complete
- **Task:** doc comments updated; stale docs pruned; new public APIs
  documented.
- **Spec:** surrounding repo docs (READMEs, guides) reflect the new
  state; outdated references removed; references to new features
  added.

---

## 4. Configuration

Pillars live in `tanren/rubric.yml` (preferred) or under
`tanren.yml` `methodology.rubric.pillars` as the canonical fallback.
Each entry:

```yaml
methodology:
  rubric:
    pillars:
      - id: completeness
        name: Completeness
        task_description: |
          Acceptance criteria met; downstream prerequisites ready;
          no silent deferrals.
        spec_description: |
          All tasks Complete; zero silent deferrals; all acceptance
          criteria verified by demo.
        target_score: 10
        passing_score: 7
        applicable_at: [task, spec]
      - id: observability
        name: Observability
        task_description: "Metrics/traces emitted for new code paths."
        spec_description: "Spec surface is debuggable in production."
        target_score: 10
        passing_score: 7
        applicable_at: [spec]
```

Users can:
- **Add** new pillars (as above).
- **Override** built-ins by re-declaring with the same id.
- **Remove** built-ins via `methodology.rubric.disable_builtin: [id, …]`.

Audit commands discover the effective pillar list via
`list_pillars_for_scope(scope)`.

---

## 5. Non-negotiables

Non-negotiables are spec-level **hard** compliance checks distinct
from scored pillars. They capture commitments from `shape-spec` that
must always hold (e.g., "must not break CLI backward compatibility",
"must not introduce unsafe code").

Recorded via `record_non_negotiable_compliance(name, status,
rationale)`. `status ∈ {pass, fail}`. Any `fail` is a hard rubric
fail regardless of pillar scores.

Non-negotiables live in the `SpecFrontmatter`; audit commands iterate
and check each.

---

## 6. Finding-score linkage

Every finding with `pillar` set contributes to that pillar's score
rationale. `record_rubric_score` validates that `supporting_finding_ids`
resolve to existing findings and that each supporting finding has
either:
- `pillar = this pillar`, AND
- `severity ∈ {fix_now, defer}` matching the linkage rule above.

This prevents "security: 2 with empty findings" anti-patterns and
makes every gap concrete and actionable.

---

## 7. Task-level vs spec-level scope

### 7.1 Task-level (audit-task)

- Scope: the single task's diff + its acceptance criteria.
- Pillars: those with `task` in `applicable_at`.
- Non-negotiables: typically N/A at task level (they're spec-wide).
- Output: task-scoped `audit.md` OR entry appended to the spec's
  rolling audit log (impl decision in `tanren-app-services::
  methodology`). Preferred: per-task audit record in the events;
  `audit.md` reflects the latest spec-level audit.

### 7.2 Spec-level (audit-spec)

- Scope: full spec's accumulated diff + demo results + task
  completion state.
- Pillars: all applicable.
- Non-negotiables: required.
- Output: `audit.md` rewritten per cycle.

---

## 8. Rubric pass result: what happens next

| Result | Orchestrator action |
|---|---|
| Pass | Advance task (task-scope) or proceed to walk-spec (spec-scope). |
| Fail with `fix_now` findings | Dispatch `investigate`; task-scoped failures record root cause and return to `do-task` for same-task repair, while spec-scoped failures may create follow-up tasks. |
| Fail with only `defer` findings | Impossible — fail requires at least one `fix_now` or a failed non-negotiable / failed demo. |
| Pass with some pillars < target but ≥ passing | Accept; defer findings to backlog (`create_issue`). |

---

## 9. Property invariants (test-enforced)

1. For every `record_rubric_score` that stores successfully:
   `score < target ⇒ supporting_finding_ids.len() > 0`.
   `score < passing ⇒ ∃ finding in supporting_finding_ids with severity = fix_now`.
2. Rubric pass iff all four pass-criteria (§2.3) hold.
3. For every finding with `pillar = X`, there exists a rubric score
   for pillar `X` unless the finding is severity `note` or `question`.
4. Disabling a built-in pillar removes its rows from rubric passes;
   a disabled pillar cannot later affect outcomes.

---

## 10. Extension checklist

To add a new pillar:
1. Add a row under `rubric.pillars` in `tanren/rubric.yml`.
2. Optionally implement a custom evaluator in
   `tanren-app-services::methodology::rubric::evaluators` if heuristic
   scoring support is desired.
3. Update audit command prose if the pillar needs command-level
   guidance beyond the `spec_description` / `task_description`.
4. Audit runs automatically incorporate it.

To add a new guard (e.g., `SecurityReviewed`):
1. Add `TaskSecurityReviewed` event variant in
   `tanren-domain::methodology::events`.
2. Add `security_reviewed` to `task_complete_requires` in
   `tanren.yml`.
3. Add a new agentic phase (command) that records
   `TaskSecurityReviewed` on pass.
4. No other code paths change; `TaskCompleted` fires when the
   guard-requirement set is satisfied.

---

## 11. See also

- Adherence (standards-based checks, not scored):
  [adherence.md](adherence.md)
- Tool surface for rubric recording:
  [agent-tool-surface.md](agent-tool-surface.md)
- Evidence file housing the rubric output:
  [evidence-schemas.md](evidence-schemas.md)
