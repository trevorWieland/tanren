# Do Task

Pick up the next unchecked task from the plan, implement it, verify it, and commit it. One task per invocation. Fully autonomous — no user interaction.

**Suggested model:** Fast, capable implementer (e.g., Sonnet via headless CLI). Cost-efficiency matters — this runs many times.

## Important Guidelines

- Autonomous execution — no shaping or interviewing
- One task per invocation — implement, verify, commit, exit
- **CRITICAL: `make check` must pass before you mark a task complete.** Run it, read every error, fix them all, and re-run until green. The orchestrator re-runs this gate externally — if it fails, you'll be called back to fix it, wasting a full cycle.
- Read signposts before starting — but verify claims against their evidence before trusting them
- Never modify spec.md — acceptance criteria and non-negotiables are immutable
- Keep changes scoped to the current task

## Prerequisites

1. A spec folder exists with `spec.md` and `plan.md`
2. Task 1 (Save Spec Documentation) is already complete
3. A branch exists and is checked out

If missing, exit with `do-task-status: error` and explain what's needed.

## Process

### Step 1: Resolve the Spec

Preferred inputs: spec folder path, issue number/URL, or `spec_id`.

Resolution order:
1. Use spec folder from input if provided.
2. Read `plan.md` for `spec_id` and `issue:` metadata.
3. If ambiguous, exit with `do-task-status: error`.

### Step 2: Load Context

Read these files (do not ask the user for summaries):

- **spec.md** — understand acceptance criteria and non-negotiables
- **plan.md** — find the first unchecked `[ ]` task
- **standards.md** — know what standards apply
- **signposts.md** (if it exists) — learn from previous errors. For each signpost, check whether the evidence supports the conclusion before relying on it.

### Step 3: Identify the Task

Find the first unchecked `[ ]` task in plan.md.

- If the task has indented `[ ]` fix items beneath it (added by audit-task), address those fix items as part of implementing the task.
- If no unchecked tasks remain, exit with `do-task-status: all-done`.

### Step 4: Implement

Execute the task as described in plan.md:

- Follow the concrete steps and referenced files listed under the task
- Adhere to standards from standards.md
- Respect non-negotiables from spec.md
- Keep changes scoped to this task only — do not refactor unrelated code

### Step 5: Run Task Gate

Run the project's task-level verification gate to catch obvious issues:

```
make check
```

This typically runs format, lint, typecheck, and unit tests.

- If it fails: read the error output, fix the issue, and re-run. Repeat until green. This self-fix avoids a full loop restart for trivial issues like formatting or type errors.
- If stuck on a verification failure after reasonable attempts: write a signpost with the exact error output as evidence, and exit as blocked.

### Step 6: Check Off Task

Update plan.md: change `[ ]` to `[x]` for the completed task.

If the task had fix items, check those off too.

### Step 7: Signpost (If Needed)

If you encountered any non-obvious issues, workarounds, or dead ends during implementation, write a signpost to signposts.md. Every signpost must include:

- **Task:** which task number
- **Status:** `unresolved` | `resolved` | `deferred` (machine-readable — auditors use this to avoid re-opening resolved issues)
- **Problem:** what went wrong
- **Evidence:** the exact error message, command output, or code snippet that demonstrates the problem
- **Tried:** what you attempted
- **Solution:** what worked (or "unresolved" if blocked)
- **Resolution:** who/what resolved it and when (e.g., "do-task round 2", "user via resolve-blockers 2026-02-08"). Omit if unresolved.
- **Files affected:** which files were involved

Do not write signposts for routine work. Only write them when future tasks or iterations would benefit from knowing what happened.

If signposts.md doesn't exist, create it with the standard header:

```markdown
# Signposts

Errors, dead ends, and non-obvious solutions encountered during implementation.
Read this before starting any task to avoid repeating known issues.

**Rule: every signpost must include evidence.** A conclusion without proof
will mislead future iterations. Include the exact error, command, or output
that demonstrates the problem.

---
```

### Step 8: Commit

Create a descriptive commit scoped to this task:

```
git add [relevant files]
git commit -m "Task N: [description]"
```

Include plan.md (updated checkbox) and signposts.md (if modified) in the commit.

### Step 9: Exit

Print one of these exit signals (machine-readable):

- `do-task-status: complete` — task done, committed
- `do-task-status: blocked` — stuck, signpost written with evidence
- `do-task-status: all-done` — no unchecked tasks remain in plan.md
- `do-task-status: error` — prerequisites missing or unrecoverable issue

## Does NOT

- Run the demo (that's run-demo)
- Audit its own work (that's audit-task)
- Create branches, push, or touch GitHub issues
- Modify spec.md (acceptance criteria and non-negotiables are immutable)
- Update roadmap.md
- Ask the user questions (fully autonomous)

## Workflow

```
shape-spec → [orchestrator: do-task ↔ audit-task loop → run-demo → audit-spec] → walk-spec → PR
```

Next step after this command: the orchestrator runs `make check` as a hard gate, then invokes `audit-task`.
