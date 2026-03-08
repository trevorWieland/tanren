# Run Demo

Execute the demo plan and validate it works. If it fails, investigate and route issues back to the task loop. Fully autonomous — no user interaction.

**Suggested model:** Capable implementer with good debugging skills (e.g., Sonnet via headless CLI).

## Important Guidelines

- Execute the demo — don't just read it and say it looks good
- **[RUN] steps MUST be executed.** You do not get to decide a [RUN] step "can't run" — that decision was made during shape-spec, and the environment was verified at that time. If a [RUN] step fails to execute, that's a FAIL and a blocker.
- **[SKIP] steps** were marked during shaping because the environment couldn't support them (probe failed). Skip them entirely — do not attempt to execute, do not count toward pass or fail.
- If a step fails, investigate the root cause before adding tasks
- If tests pass but the demo fails, that's a test gap — identify what's missing
- Never fix code directly — add tasks to plan.md for do-task to pick up
- Never modify spec.md
- Signposts must include evidence

## Prerequisites

1. A spec folder exists with `spec.md`, `plan.md`, and `demo.md`
2. All tasks in plan.md are checked off
3. The full verification gate (`make all`) has passed

If missing, exit with `run-demo-status: error` and explain what's needed.

## Process

### Step 1: Resolve the Spec

Preferred inputs: spec folder path, issue number/URL, or `spec_id`.

Resolution order:
1. Use spec folder from input if provided.
2. Read `plan.md` for `spec_id` and `issue:` metadata.
3. If ambiguous, exit with `run-demo-status: error`.

### Step 2: Load Context

Read these files:

- **demo.md** — the demo steps, environment info, and expected outcomes
- **spec.md** — acceptance criteria (for context on what the demo should prove)
- **signposts.md** (if it exists) — check for known issues that might affect the demo

Pay attention to the `## Environment` section in demo.md — it documents what's available (API keys, services, setup). If setup steps are listed, run them before starting the demo.

### Step 3: Execute Demo

Process each demo step according to its classification:

#### For **[RUN]** steps (the default):

1. Read the step description and expected outcome
2. **Execute the action** (run the command, make the config change, call the API, etc.)
3. Observe the result
4. Compare against the expected outcome
5. Record as PASS or FAIL

A [RUN] step that you cannot execute is a **FAIL**, not a skip. The environment was assessed during shape-spec — if the step is marked [RUN], the environment supports it. If something is genuinely broken (missing dependency, service down), that's a failure to investigate and route back to the task loop.

#### For **[SKIP]** steps:

1. Note the step and its reason for skipping (documented in demo.md)
2. Record as SKIPPED in the results
3. Do not attempt to execute — the environment was verified as unable to support this step during shaping

[SKIP] steps do not count toward pass or fail. A demo passes when all [RUN] steps pass and at least one [RUN] step exists.

#### For steps with no tag (legacy demo.md without classifications):

Treat all steps as [RUN]. Execute them. If you genuinely cannot execute a step, report it as FAIL with the reason, not as SKIP.

**If a step passes:** Note it and continue to the next step.

**If a step fails:** Stop executing further steps and proceed to Step 4 (Investigation).

### Step 4: Investigate Failures

For each failed step:

1. **Read the error output** — what exactly went wrong?
2. **Check test coverage** — do existing tests cover the failing scenario?
   - If tests pass but the demo fails, this is a **test gap**. The test suite should guarantee the demo works. Identify what specific tests are missing.
3. **Trace the root cause** — read the relevant code, follow the execution path, identify where the behavior diverges from the expected outcome.
4. **Write a signpost** in signposts.md with:
   - **Task:** which task/step failed
   - **Status:** `unresolved` (always unresolved when first written by run-demo)
   - **Problem:** the exact error or unexpected output
   - **Evidence:** what the expected outcome was vs what happened
   - **Root cause:** the root cause (or best hypothesis if uncertain)
   - **Files affected:** which files/functions are involved
5. **Add tasks to plan.md** for do-task to fix:
   - If a test gap was found: add a task to write the missing test(s) first, then a task to fix the underlying issue
   - If the code is simply wrong: add a task describing the fix needed
   - Each new task must be specific enough for do-task to act on without ambiguity

### Step 5: Record Results

Append results to demo.md under `## Results`:

```markdown
### Run N — [context] (YYYY-MM-DD HH:MM)
- Step 1 [RUN]: PASS|FAIL — [brief note]
- Step 2 [RUN]: PASS|FAIL — [brief note]
- Step 3 [SKIP]: SKIPPED — [reason from demo.md]
- **Overall: PASS|FAIL**
```

Overall result rules:
- **PASS** requires: all [RUN] steps PASS, at least one [RUN] step exists
- **FAIL** if: any [RUN] step fails
- [SKIP] steps are recorded but do not affect the overall result

If this is the first run, add the `## Results` header.

### Step 6: Update Audit Log

Append a brief entry to audit-log.md:

```
- **Demo** (run N): PASS|FAIL — [one-line summary] ([X run, Y verified])
```

If audit-log.md doesn't exist, create it with the standard header.

### Step 7: Commit

Commit demo.md, audit-log.md, signposts.md, and plan.md changes:

```
git add demo.md audit-log.md signposts.md plan.md
git commit -m "Demo run N: PASS|FAIL"
```

Only include files that were actually modified.

### Step 8: Exit

Print one of these exit signals (machine-readable):

- `run-demo-status: pass` — all [RUN] steps passed ([SKIP] steps excluded)
- `run-demo-status: fail` — one or more [RUN] steps failed, tasks added to plan.md
- `run-demo-status: error` — prerequisites missing or unrecoverable issue

## Does NOT

- Fix code itself (it adds tasks for do-task to pick up)
- Modify spec.md
- Push or touch GitHub
- Score rubrics (that's audit-spec)
- Skip investigation — every failure must be diagnosed before adding tasks
- Reclassify steps — [RUN] vs [SKIP] was decided during shape-spec and is binding

## Workflow

```
shape-spec → [orchestrator: do-task ↔ audit-task loop → run-demo → audit-spec] → walk-spec → PR
```

Next step after this command: if pass, the orchestrator invokes audit-spec. If fail, the orchestrator loops back to do-task to address the newly added tasks.
