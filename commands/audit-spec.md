# Audit Spec

Bird's-eye audit of the full implementation after all tasks and demo pass. Score rubrics, check non-negotiables, verify standards adherence, detect regressions. Fully autonomous — no user interaction.

**Suggested model:** Strong reasoner, different from do-task (e.g., Codex high reasoning via headless CLI). Deep review — quality matters more than speed.

## Important Guidelines

- This is the full-picture audit — check everything, not just the latest task
- Non-negotiables are hard pass/fail — no exceptions, no caveats
- Be specific in citations — file:line and standard rule references
- Write machine-readable status in audit.md so the orchestrator can parse it
- Never modify spec.md
- Prefer GitHub issues for deferrals instead of editing roadmap.md

## Prerequisites

1. A spec folder exists with `spec.md`, `plan.md`, `standards.md`, and `demo.md`
2. All tasks in plan.md are checked off
3. Demo has been run (results in demo.md)
4. If the full verification gate wasn't run, note it in the audit instead of stopping

## Process

### Step 1: Resolve the Spec

Preferred inputs: spec folder path, issue number/URL, or `spec_id`.

Resolution order:
1. Use spec folder from input if provided.
2. Read `plan.md` for `spec_id` and `issue:` metadata.
3. If ambiguous, exit with error.

### Step 2: Load Context

Read all spec files — do not ask the user for summaries:

- **spec.md** — acceptance criteria, non-negotiables (the contract)
- **plan.md** — tasks, fix items, completion status
- **standards.md** — applicable standards
- **references.md** — related files and resources
- **demo.md** — demo plan and results
- **signposts.md** — known issues and workarounds
- **audit-log.md** — history of task audits and demo runs (check for regressions)
- **Implementation files and tests** referenced by the plan

### Step 3: Perform Audit

Evaluate the full implementation across these dimensions:

#### 3a: Rubric Scores (1-5)

Score each dimension:

- **Performance** — Is the implementation efficient? No unnecessary computation, I/O, or memory usage?
- **Intent** — Does the implementation match the spec's stated goals? Does it solve the right problem?
- **Completion** — Are all acceptance criteria met? Any gaps or partial implementations?
- **Security** — No injection vulnerabilities, hardcoded credentials, unsafe deserialization, or other OWASP concerns?
- **Stability** — Proper error handling, no silent failures, resilient to edge cases?

#### 3b: Non-Negotiable Compliance

Check every non-negotiable from spec.md's **Note to Code Auditors** section. For each one:

- Explicitly state **PASS** or **FAIL**
- Cite evidence (file:line reference, code snippet, or command output)

Non-negotiable failures are automatic audit failures regardless of rubric scores.

#### 3c: Standards Adherence

Check all applicable standards from standards.md across the full implementation (not just per-task diffs). For each violation:

- Cite the standard rule
- Cite the file:line where the violation occurs
- Classify severity: High / Medium / Low

#### 3d: Demo Status

Check demo.md Results:

- Did the demo pass?
- Are the results convincing?
- Were any steps skipped? If so, is the justification reasonable?

If the demo was not run, flag it as a **Fix Now** action item.

#### 3e: Signpost Cross-Reference

Before flagging any issue as a Fix Now item, cross-reference signposts.md **and** existing open fix items in plan.md:

1. **Resolved signposts** (`Status: resolved`): Verify the resolution is implemented in code. If it is — do NOT re-open the issue as a Fix Now item. If you believe the resolution is insufficient, you must provide **new evidence** (not the original problem) showing why it fails. Asking do-task to undo a documented resolution without counter-evidence wastes cycles.
2. **Architectural constraints**: If a signpost documents that approach X is infeasible (with evidence of why), do NOT add a Fix Now item that requires approach X. Either propose an alternative approach or defer it.
3. **Deferred signposts** (`Status: deferred`): These were explicitly deferred — do not promote them to Fix Now unless new evidence shows they're blocking.
4. **Unresolved signposts**: These are fair game — they may warrant Fix Now items if they affect spec compliance.
5. **Existing open fix items**: Scan plan.md for unchecked `[ ] Fix:` entries. If one already covers the same issue (same file, same problem), do NOT add a duplicate — the existing item will be addressed in the next do-task cycle.

#### 3f: Regression Check

Review audit-log.md for patterns:

- Tasks that failed and were fixed — have they regressed?
- Recurring issues across multiple tasks
- Signposts that suggest systemic problems

#### 3g: Cross-Cutting Concerns

Things that only emerge at the full-picture level:

- Consistency across files (naming, patterns, conventions)
- Architectural coherence
- Test coverage gaps that span multiple tasks

### Step 4: Categorize Action Items

Use the default routing:

```
High/Medium severity → Fix Now
Low severity → Defer
```

**Fix Now items — grouping rules:**

Fix items must be grouped under task headings so the orchestrator can assign
them one group at a time. Never append bare fix items to the bottom of the
file — they become orphaned and the task loop cannot track them.

1. **Deduplicate first.** Scan existing unchecked `[ ] Fix:` items in plan.md.
   If an open fix item already describes the same issue (same file, same
   problem), do NOT add a duplicate. Move on.

2. **Route to the relevant task.** For each new fix item, identify which
   existing task is responsible for the code/test it targets:
   - Uncheck the task: `[x]` → `[ ]`
   - Append the fix item as an indented `[ ] Fix:` entry under that task
   - Include file:line citations, the audit round, and a clear fix description
   ```
   - [ ] Task 7: `rentl benchmark` CLI subcommands
     - [x] (existing sub-items stay as-is)
     - [ ] Fix: Remove dead `_run_benchmark_async` placeholder path (`main.py:2590`) (audit round 3)
   ```

3. **Create a new task group** only if a fix item genuinely doesn't belong to
   any existing task. Use the next sequential task number and full formatting:
   ```
   - [ ] Task 10: <short imperative description>
     - [ ] Fix: <specific item with file:line> (audit round N)
     - [ ] Fix: <another item if needed> (audit round N)
   ```

**Deferred items:**
1. Determine the next available `spec_id` (see `tanren/product/github-conventions.md` → Resolving spec_id).
2. Create a GitHub issue with YAML frontmatter body (Format A):
   ```bash
   gh issue create \
     --title "{spec_id} {short title}" \
     --label "type:spec" --label "status:planned" --label "version:vX.Y" \
     --milestone "{version}" \
     --body-file /tmp/issue_body.md
   ```
   Where `/tmp/issue_body.md` contains:
   ```markdown
   ---
   spec_id: sX.Y.ZZ
   version: vX.Y
   status: planned
   depends_on: [{current_spec_id}]
   ---

   Deferred from {current_spec_id} audit (round N).

   {context and description of deferred work}

   ## References

   - Source spec: {current_issue_url}
   ```
3. Add `blockedBy` relationship linking the new issue to the current spec issue via GraphQL (see `tanren/product/github-conventions.md` → Dependency Relationships).
4. Record the new issue URL in audit.md.

Do **not** edit roadmap.md during audit.

### Step 5: Write audit.md

Write audit.md with a machine-readable header followed by the full audit report:

```markdown
status: pass|fail
fix_now_count: N

# Audit: {spec_id} {title}

- Spec: {spec_id}
- Issue: {issue_url}
- Date: YYYY-MM-DD
- Round: N

## Rubric Scores (1-5)
- Performance: N/5
- Intent: N/5
- Completion: N/5
- Security: N/5
- Stability: N/5

## Non-Negotiable Compliance
1. [Non-negotiable 1]: **PASS|FAIL** — [evidence with file:line]
2. [Non-negotiable 2]: **PASS|FAIL** — [evidence with file:line]

## Demo Status
- Latest run: PASS|FAIL (Run N, date)
- [Summary of results]

## Standards Adherence
- [Standard]: [PASS|violation with file:line citation]

## Regression Check
- [Findings from audit-log.md review]

## Action Items

### Fix Now
- [Item with file:line citation]

### Deferred
- [Item] → [GitHub issue URL]
```

**Status rules:**
- `status: pass` requires: all rubric scores 5/5, all non-negotiables PASS, demo PASS, zero Fix Now items
- `status: fail` if any of the above conditions aren't met

### Step 6: Update Audit Log

Append a brief entry to audit-log.md:

```
- **Spec Audit** (round N): PASS|FAIL — [rubric summary, fix-now count]
```

### Step 7: Report to GitHub

Update the existing "Spec Progress" comment on the spec issue (create it once if missing) with:

- Overall status and score
- Fix Now count
- Deferred issue links
- Demo status

### Step 8: Commit

Commit audit.md, audit-log.md, and plan.md changes:

```
git add audit.md audit-log.md plan.md
git commit -m "Spec audit round N: PASS|FAIL"
```

### Step 9: Exit

The exit signal is the `status:` field in audit.md (machine-readable):

- `status: pass` → orchestrator proceeds to notify human for walk-spec
- `status: fail` → orchestrator loops back to do-task to address Fix Now items

## Does NOT

- Fix anything (adds Fix Now items to plan.md for do-task)
- Run the demo itself (reads results from demo.md)
- Push or create PRs
- Touch roadmap.md
- Modify spec.md

## Success Criteria

All rubric scores are 5/5, all non-negotiables pass, demo passes, and no Fix Now items remain.

## Workflow

```
shape-spec → [orchestrator: do-task ↔ audit-task loop → run-demo → audit-spec] → walk-spec → PR
```

Next step after this command: if pass, the orchestrator notifies the human to run walk-spec. If fail, the orchestrator loops back to do-task.
