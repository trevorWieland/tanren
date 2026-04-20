# Adherence

Authoritative spec for the standards-adherence check phases
(`adhere-task`, `adhere-spec`). Adherence is the mechanical
compliance layer that sits beside `audit-*` and `*-gate` phases as
one of the required forward guards on every task.

Companion docs:
[audit-rubric.md](audit-rubric.md) (opinionated judgment),
[orchestration-flow.md](orchestration-flow.md),
[agent-tool-surface.md](agent-tool-surface.md).

---

## 1. Why adherence is separate

| Concern | Audit | Adherence |
|---|---|---|
| Question asked | "Is this good work?" | "Does this follow our rules?" |
| Output shape | Scored pillars (1–10) | Pass/fail per standard |
| Source of truth | Hardcoded + extensible pillar taxonomy | Repo-authored standards files |
| Cadence | Every task + spec | Every task + spec |
| Judgment required | Yes | No (deterministic given the standards) |
| Filtering | All applicable pillars | Only standards relevant to this scope |

Keeping them separate avoids flattening nuanced quality judgment into
pass/fail compliance (and vice versa).

---

## 2. Relation to triage-audits

| Command | When it runs | Scope | Output |
|---|---|---|---|
| `adhere-task` | Every task, as a required guard | The task's diff | Findings, loops task |
| `adhere-spec` | Every spec, as a required guard | The spec's full diff | Findings, loops to more tasks |
| `triage-audits` | On demand / periodic batch | Whole codebase via `audit-standards.sh` | Backlog issues for future specs |

Adherence keeps standards compliance **continuous** during spec
execution. Triage-audits is a **curatorial** cross-spec pass for
backlog grooming. Both use standards, but at different granularity and
cadence.

---

## 3. Phase definitions

### 3.1 adhere-task

- **Autonomy:** autonomous
- **Input:** supplied `task_id` + diff range for the task
- **Tools:** `list_relevant_standards`, `record_adherence_finding`,
  `list_tasks`, `report_phase_outcome`
- **Output:**
  - Zero or more adherence findings with severity `fix_now` or
    `defer`.
  - Phase outcome `complete` (passes the `Adherent` guard iff zero
    `fix_now` adherence findings).
- **Guard emitted on success:** `TaskAdherent`
- **Failure routing:** orchestrator creates new tasks from `fix_now`
  findings (`origin: Adherence { source_standard, source_finding }`);
  task's `Adherent` guard remains unsatisfied until a subsequent
  adherence pass produces zero `fix_now`.

### 3.2 adhere-spec

- **Autonomy:** autonomous
- **Input:** spec folder + spec-wide diff
- **Tools:** same as `adhere-task`
- **Output:** spec-level adherence findings
- **Guard:** `SpecAdherent` (spec-level analog; Lane 0.5 models the
  required spec-guard set analogously to tasks)
- **Failure routing:** orchestrator creates new tasks (origin
  `Adherence`) which loop the task state machine.

---

## 4. Relevant-standard filtering

### 4.1 Algorithm

Input: `spec_id` (authoritative) plus optional caller hints
(`touched_files`, `project_language`, `domains`, `tags`, `category`).
Server-side relevance is derived from persisted spec state first, then
hint values are unioned in as additive context.

```
relevant(standard):
    if standard.applies_to globs intersect effective.touched_files:
        return true
    if standard.applies_to_languages contains any effective.project_languages:
        return true
    if standard.applies_to_domains contains any effective.domains:
        return true
    if standard has no applies_to/applies_to_languages/applies_to_domains:
        return true
    return false

standards = discover_all_standards(STANDARDS_ROOT)
return [s for s in standards if relevant(s)]
```

`effective.*` is built as:
- `effective.touched_files = spec.relevance_context.touched_files ∪ caller.touched_files`
- `effective.project_languages = {spec.relevance_context.project_language?, caller.project_language?}`
- `effective.domains = spec.relevance_context.tags ∪ spec.relevance_context.category? ∪ caller.domains ∪ caller.tags ∪ caller.category?`

Caller hints can broaden relevance, but cannot narrow spec-derived context.
Path matching uses full glob semantics (globset) after path
normalization (`\` → `/`, strip leading `./`).

### 4.2 Standard metadata (required on every standard file)

```yaml
---
name: <string>
category: <string>             # e.g. "api", "testing", "style"
applies_to: [<glob>]           # e.g. ["**/*.rs", "crates/tanren-*/src/**"]
applies_to_languages: [<string>]  # e.g. ["rust", "typescript"]
applies_to_domains: [<string>]    # e.g. ["storage", "api-surface"]
importance: <low|medium|high|critical>
---
<standard body: the rule, examples of compliance and violation>
```

---

## 5. Adherence finding shape

Distinct from audit findings (no `pillar`; has `standard_ref`):

```
record_adherence_finding inputs:
  standard: { category: <string>, name: <string> }  // must exist in runtime standards registry
  severity: <fix_now | defer>        // note/question not used for adherence
  affected_files: [<path>]
  line_numbers?: [<u32>]
  rationale: <string>                // why this is a violation
```

Stored as `AdherenceFindingAdded` event with
`FindingSource::Adherence { standard: StandardRef }`.
Unknown `(category, name)` pairs are rejected at the tool boundary
with typed remediation to list standards first.

---

## 6. Severity policy

- **`fix_now`:** violation blocks the `Adherent` guard; must be fixed
  before task/spec completes.
- **`defer`:** violation is real but acceptable to defer per the
  standard's `importance` or the auditor's judgment. Emits a backlog
  `create_issue` via the orchestrator.

Standards with `importance: critical` cannot produce `defer`
findings; `record_adherence_finding` with `severity: defer` on a
critical standard returns `ToolError::ValidationFailed` with
remediation `"Critical standards cannot be deferred; use fix_now or
demonstrate compliance"`.

---

## 7. Tool capability scope

Adherence phases receive:
```
standard.read
adherence.record
task.read
phase.outcome
```

Notably absent: `task.create`, `rubric.record`, `compliance.record`,
`finding.add`. The
orchestrator is the sole mutator of task state post-adherence;
rubric scoring belongs to `audit-*`; generic `add_finding` isn't
needed because adherence has its own typed channel.

---

## 8. Output evidence

Adherence findings accumulate in the store and are referenced from
`audit.md` (under the `adherence` section) and from `plan.md`
(where new-task-origin metadata shows `Adherence { standard_id }`).

There is no separate `adherence.md` evidence doc for Lane 0.5;
downstream lanes may add one if per-standard summary reports become
useful.

---

## 9. Property invariants

Test-enforced in `tanren-app-services::methodology::adherence`:

1. `list_relevant_standards(spec_id)` returns only standards where
   `relevant(standard)` is true for the server-derived spec relevance
   context plus additive caller hints.
2. Every adherence finding links to a standard that existed at the
   time of recording.
3. Critical-importance standards can never produce `defer` findings.
4. Task's `Adherent` guard is satisfied iff a successful
   `adhere-task` run for that task recorded zero `fix_now`
   findings since the last `TaskImplemented` event.

---

## 10. Extension and evolution

- **New standard** → user drops a file under `STANDARDS_ROOT` with
  required metadata; next adherence run picks it up.
- **New adherence scope** (e.g., library-wide rather than per-file) →
  add new `applies_to_*` fields to the standard schema; update the
  relevance algorithm.
- **Severity changes** require an explicit contract version bump
  because v1 adherence is intentionally strict (`fix_now|defer` only).

---

## 11. See also

- Audit rubric (scored judgment): [audit-rubric.md](audit-rubric.md)
- Orchestration flow integration:
  [orchestration-flow.md](orchestration-flow.md)
- Tool surface: [agent-tool-surface.md](agent-tool-surface.md)
- Design rationale: [../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md](../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md)
