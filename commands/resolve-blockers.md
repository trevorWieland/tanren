# Resolve Blockers

Investigate blockers that halted the orchestrator, propose solutions, and reconcile the spec artifacts after the user chooses a path forward. Interactive — the user makes decisions, the agent investigates and executes.

**Suggested model:** Strong reasoner with good interactive skills (e.g., Opus via TUI). Needs to understand architectural constraints and present options clearly.

## Important Guidelines

- Human-in-the-loop — investigate and present options, but the user decides
- Always use AskUserQuestion tool when asking the user anything. In TUI: use AskUserQuestion. In Discord/NanoClaw: use send_message with numbered options and wait for reply.
- Never modify spec.md without explicit user approval (and even then, present the exact diff first)
- Signposts must include evidence — verify every claim before trusting it
- Changes must be committed with clear "Resolve blockers" messages
- This command can update plan.md, signposts.md, and (with approval) spec.md

## When to Use

- After the orchestrator halts with "Human intervention needed. See signposts.md."
- After the orchestrator halts with "Task stuck after N attempts"
- After the orchestrator halts with "Stale — plan.md unchanged"
- When the user notices audit contradictions (e.g., auditor ignoring resolved signposts)
- Proactively, when plan.md has fix items that conflict with signpost resolutions

## Prerequisites

1. A spec folder exists with `spec.md`, `plan.md`, and `signposts.md`
2. At least one blocker exists (unresolved signpost, stuck task, stale loop, or contradictory fix items)

If no blockers are found, explain this and exit with `resolve-blockers-status: no-blockers`.

## Process

### Step 1: Resolve the Spec

Preferred inputs: spec folder path, issue number/URL, or `spec_id`.

Resolution order:
1. Use spec folder from input if provided.
2. Read `plan.md` for `spec_id` and `issue:` metadata.
3. If ambiguous, ask the user.

### Step 2: Investigate

Read all spec artifacts:

- **spec.md** — acceptance criteria and non-negotiables (the contract)
- **plan.md** — tasks, fix items, completion status
- **signposts.md** — blockers, resolutions, architectural constraints
- **audit-log.md** — history of audits and patterns
- **standards.md** — applicable standards

Then identify all blockers by scanning for:

1. **Unresolved signposts** — entries with `Status: unresolved` or no Status field
2. **Contradictory fix items** — unchecked fix items in plan.md that conflict with resolved signposts (e.g., a fix item asks to undo something a signpost says is resolved)
3. **Stuck tasks** — tasks that have been retried multiple times (check audit-log.md for repeated FAIL entries on the same task)
4. **Architectural constraints** — signposts documenting that certain approaches are infeasible
5. **Spec/reality mismatches** — cases where the spec.md acceptance criteria require something that's architecturally impossible (evidenced by signposts)

For each blocker, read the relevant code at the cited file:line locations to verify the claims.

### Step 3: Present Blocker Report

Present a structured summary to the user:

```
## Blocker Report

### Blocker 1: [short description]
- **Source:** [signpost / plan.md fix item / audit-log pattern]
- **Root cause:** [what's actually wrong, verified against code]
- **What's been tried:** [from signposts and audit-log]
- **Architectural constraints:** [if any, from signposts]
```

For each blocker, present 2-3 resolution strategies ranked by recommendation:

```
### Resolution Options for Blocker 1

**Option A (Recommended): [short label]**
- What: [concrete description of what changes]
- Why: [rationale — why this is the best path]
- Changes: [which files would be modified]
- Risk: [any downsides]

**Option B: [short label]**
- What: ...
- Why: ...
- Changes: ...
- Risk: ...

**Option C: Defer to future spec**
- What: Create a GitHub issue, mark signpost as deferred, remove contradictory fix items
- Why: [when this makes sense]
```

Use AskUserQuestion to let the user pick an option per blocker. Group related blockers if they share a root cause.

### Step 4: Reconcile Artifacts

Based on the user's decisions, update the spec artifacts:

#### For each resolved blocker:

1. **Update signposts.md:**
   - If the signpost exists: update its `Status` to `resolved` or `deferred`
   - Add `Resolution:` field with who resolved it and when (e.g., "user via resolve-blockers 2026-02-08")
   - If no signpost exists for this blocker: write one with the full evidence and resolution

2. **Update plan.md:**
   - Remove contradictory fix items that conflict with the chosen resolution
   - Add new fix items if the resolution requires implementation work
   - If a task needs to be re-scoped, update the task description (but NOT the task number)

3. **Update spec.md (ONLY with explicit user approval):**
   - If the resolution involves accepting that an acceptance criterion is architecturally infeasible, present the exact proposed edit to the user first
   - Show a before/after diff of the affected criterion
   - Only make the edit after the user explicitly approves
   - This is the nuclear option — prefer adjusting plan.md or deferring over changing spec.md

#### For deferred blockers:

1. Determine the next available `spec_id` (see `tanren/product/github-conventions.md` → Resolving spec_id).
2. Create a GitHub issue with YAML frontmatter body (Format A):
   ```bash
   gh issue create \
     --title "{spec_id} {short description of deferred work}" \
     --label "type:spec" --label "status:planned" --label "version:{version}" \
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

   Deferred from {current_spec_id} during blocker resolution.

   {context and architectural constraint}

   ## References

   - Source spec: {current_issue_url}
   ```
3. Add `blockedBy` relationship linking the new issue to the current spec issue via GraphQL (see `tanren/product/github-conventions.md` → Dependency Relationships).
4. Update signpost `Status` to `deferred`
5. Remove any fix items in plan.md that depend on the deferred work
6. Record the issue URL in the signpost

### Step 5: Verify Consistency

After all updates, check for internal consistency:

1. No unchecked fix items that contradict resolved signposts
2. No checked-off tasks that have unresolved fix items below them
3. All signposts have a Status field
4. plan.md task count matches what's expected

If inconsistencies are found, fix them and explain what was corrected.

### Step 6: Run Gate (If Implementation Changes Were Made)

If the resolution involved code changes (not just artifact updates):

```
make check
```

If it fails, investigate and fix. If the fix is non-trivial, add a task to plan.md for do-task to handle.

### Step 7: Commit

Commit all changes:

```
git add plan.md signposts.md spec.md audit-log.md
git commit -m "Resolve blockers: [brief summary of what was resolved]"
```

Only include files that were actually modified.

### Step 8: Advise Next Steps

Tell the user what to do next:

- If new fix items were added: "Re-run the orchestrator to address the new tasks."
- If only artifacts were reconciled: "Re-run the orchestrator — it should proceed past the previous blocker."
- If spec.md was modified: "Note: spec.md was updated. The orchestrator will use the new acceptance criteria."
- If items were deferred: "Deferred items tracked in GitHub issues: [URLs]"

### Step 9: Exit

Print one of these exit signals (machine-readable):

- `resolve-blockers-status: resolved` — all blockers resolved, orchestrator can resume
- `resolve-blockers-status: deferred` — some blockers deferred to future specs
- `resolve-blockers-status: partial` — some blockers resolved, others need further investigation
- `resolve-blockers-status: no-blockers` — no blockers found
- `resolve-blockers-status: error` — prerequisites missing or unrecoverable issue

## Does NOT

- Implement code fixes (adds tasks for do-task)
- Run the demo (that's run-demo)
- Audit the implementation (that's audit-task / audit-spec)
- Push or create PRs (that's walk-spec)
- Make decisions autonomously — the user decides, the agent executes

## Workflow

```
orchestrator halts → user runs resolve-blockers → resolve-blockers reconciles → user re-runs orchestrator
```

This command bridges the gap between an orchestrator halt and resuming the automated loop.
