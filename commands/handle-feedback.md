# Handle Feedback

Pull PR review comments and automated analysis feedback, evaluate their correctness, and route valid feedback into the spec workflow as tasks. Interactive — the user validates the triage before any actions are taken.

**Suggested model:** Strong reasoner with good judgment (e.g., Opus via TUI). Evaluating whether a reviewer is correct requires deep technical reasoning and the user must approve the triage.

## Important Guidelines

- Human-in-the-loop — the user approves the triage before any changes or replies are posted
- Always use AskUserQuestion tool when asking the user anything. In TUI: use AskUserQuestion. In Discord/NanoClaw: use send_message with numbered options and wait for reply.
- Never blindly accept feedback — verify every claim against the actual code
- Never blindly reject feedback — incorrect dismissals erode reviewer trust
- Never modify spec.md
- Reply comments must be respectful and evidence-based
- Signposts must include evidence

## When to Use

- After a PR is created (via walk-spec) and review comments arrive
- When automated code analysis agents leave comments on the PR
- When community reviewers provide feedback on a PR
- When CI checks (GitHub Actions) fail on the PR
- Periodically during long-running PR reviews to process new comments

## Prerequisites

1. A spec folder exists with `spec.md` and `plan.md`
2. A PR exists for the current spec branch (verifiable via `gh pr view`)
3. The PR has review comments, failed CI checks, or check annotations to process

If no PR is found, ask the user for the PR number/URL.
If no comments and no failed checks are found, exit with `handle-feedback-status: no-feedback`.

## Process

### Step 1: Resolve the Spec and PR

Preferred inputs: spec folder path, PR number/URL, issue number/URL, or `spec_id`.

Resolution order:
1. Use spec folder from input if provided.
2. Read `plan.md` for `spec_id` and `issue:` metadata.
3. Find the PR: `gh pr list --head <branch-name>` or use the provided PR number.
4. If ambiguous, ask the user.

### Step 2: Fetch Feedback

Gather all review feedback from the PR:

1. **PR review comments** (inline code comments):
   ```
   gh api repos/{owner}/{repo}/pulls/{pr_number}/comments
   ```

2. **PR review bodies** (top-level review summaries):
   ```
   gh api repos/{owner}/{repo}/pulls/{pr_number}/reviews
   ```

3. **Issue-style comments** (conversation comments on the PR):
   ```
   gh pr view {pr_number} --comments --json comments
   ```

4. **CI check failures** (GitHub Actions workflow run logs):
   ```
   gh pr checks {pr_number} --json name,state,bucket,link
   ```
   For each check where `bucket` is `"fail"`:
   - Extract the `run_id` from the `link` field (URL path segment after `/runs/`)
   - Fetch the failure logs:
     ```
     gh run view {run_id} --log-failed
     ```
   - Parse the log output to extract individual failures:
     - **Pytest:** Each `FAILED path/to/file.py::test_name - ErrorType: message` line in the short summary, plus the traceback for the originating file:line in project code
     - **Ruff:** Each `path/to/file.py:line:col: RULE message` line
     - **ty:** Each `error[code]` block with its `-->` file:line pointer
   - If `--log-failed` returns empty output, fall back to check run annotations:
     ```
     gh api repos/{owner}/{repo}/check-runs/{check_run_id}/annotations
     ```

Parse each piece of feedback into a structured list:
- **ID:** unique identifier for reference
- **Source:** author name/bot, review type (inline/review/comment/check)
- **File:line:** if applicable (inline comments and check annotations)
- **Body:** the feedback text
- **Is bot:** whether the author is a bot/automated tool

### Step 3: Load Context

Read these spec files for cross-referencing:

- **spec.md** — acceptance criteria and non-negotiables
- **plan.md** — tasks and completion status
- **signposts.md** — known issues and resolutions
- **standards.md** — applicable standards
- **audit.md** (if exists) — latest audit results

### Step 4: Triage Each Item

For each piece of feedback, perform this analysis:

1. **Read the relevant code** at the referenced file:line. Understand the context — read surrounding code, not just the cited line.

2. **Evaluate the claim:**
   - Is the reviewer's technical assertion correct?
   - Does the code actually have the issue they describe?
   - If they suggest a fix, would it work? Would it introduce other problems?
   - Check signposts.md — has this issue already been identified and resolved?

3. **Classify the feedback** into one of these categories:

   - **valid-actionable** — the feedback is correct and points to a real issue that should be fixed
   - **valid-addressed** — the feedback is correct but the issue is already handled (in code, signposts, or spec design decisions)
   - **invalid** — the reviewer is wrong (with evidence of why)
   - **style-preference** — subjective, not a correctness issue (e.g., naming choices, formatting preferences)
   - **out-of-scope** — real issue but belongs to a different spec or future work
   - **duplicate** — same issue raised by another comment already in this triage

   **CI failure fast-track:** Items from CI check failures (Step 2.4) default to `valid-actionable` since CI is objective. Override to `valid-addressed` if signposts.md already tracks the issue, or `out-of-scope` if the failure is environmental (e.g., missing CI secrets, platform-specific behavior).

4. **Write a brief rationale** for each classification, citing file:line evidence.

### Step 5: Present Triage to User

Present the triage as a structured summary, grouped by classification:

```
## Feedback Triage — PR #{pr_number}

### Valid & Actionable (N items)

1. **[Author] on [file:line]:** [brief summary of feedback]
   - **Assessment:** [why this is correct]
   - **Proposed action:** [what fix item to add]

### CI Failures (N items)

N. **[CI: {make_target}] {test_name or rule}** at `{file:line}`
   - **Error:** {error message}
   - **Assessment:** {root cause analysis}
   - **Proposed fix item:** {description for plan.md}

### Valid but Already Addressed (N items)

2. **[Author] on [file:line]:** [brief summary]
   - **Assessment:** [where/how it's already handled]
   - **Proposed reply:** [draft reply explaining this]

### Invalid (N items)

3. **[Author] on [file:line]:** [brief summary]
   - **Assessment:** [why the reviewer is wrong, with evidence]
   - **Proposed reply:** [draft reply — respectful, with code references]

### Style Preference (N items)

4. **[Author]:** [brief summary]
   - **Assessment:** [why this is subjective]
   - **Proposed action:** Note for future standards consideration

### Out of Scope (N items)

5. **[Author]:** [brief summary]
   - **Assessment:** [why this belongs elsewhere]
   - **Proposed action:** Create GitHub issue for future work

### Duplicates (N items)
- [list of duplicates referencing which item they duplicate]
```

Use AskUserQuestion to let the user:
- Override any classification (e.g., promote "invalid" to "valid-actionable" if they disagree)
- Edit draft replies before posting
- Choose which actions to take

### Step 6: Execute Approved Actions

After user approval, execute each action:

#### For valid-actionable items:

1. Add fix items to plan.md as unchecked entries:
   ```
   - [ ] Fix: [description from feedback] (PR #{pr_number} feedback from @{author}, feedback round N)
   ```
2. If the fix item reveals a non-obvious issue, write a signpost to signposts.md:
   - **Task:** which task is affected
   - **Status:** unresolved
   - **Problem:** what the reviewer found
   - **Evidence:** the reviewer's comment with file:line reference
   - **Impact:** why this matters
3. Uncheck the affected task in plan.md if a fix item is added beneath it.

#### For CI failure items (valid-actionable):

Follow the same process as valid-actionable items above, with these additions:

1. Prefix fix items with `[CI]` for traceability:
   ```
   - [ ] Fix: [CI] {description} ({file}:{line}, CI run #{run_id})
   ```
2. Group multiple failures from the same root cause into a single fix item (e.g., three test failures caused by the same missing import).
3. Include the exact error text in the signpost evidence if a signpost is warranted.

#### For valid-addressed items:

1. Post a reply comment on the PR explaining where/how the issue is handled:
   ```
   gh api repos/{owner}/{repo}/pulls/{pr_number}/comments/{comment_id}/replies \
     -f body="[reply text]"
   ```
   For non-inline comments, use:
   ```
   gh pr comment {pr_number} --body "[reply text]"
   ```
2. Reference specific file:line locations and signpost entries in the reply.

#### For invalid items:

1. Post a polite reply comment with evidence:
   - Thank the reviewer for the observation
   - Explain why the code is correct as-is (with file:line references)
   - If the confusion is understandable, acknowledge that and explain the design decision
2. If the reviewer raised a point that, while technically incorrect, reveals a readability or documentation gap, note it as a style-preference item instead.

#### For style-preference items:

1. Note them in the reply as "acknowledged, may consider for future standards"
2. If the user wants to adopt the suggestion: add a fix item to plan.md
3. If multiple reviewers raise the same style point: recommend the user run `/discover-standards` after this PR

#### For out-of-scope items:

1. Determine the next available `spec_id` (see `tanren/product/github-conventions.md` → Resolving spec_id).
2. Create a GitHub issue with YAML frontmatter body (Format A):
   ```bash
   gh issue create \
     --title "{spec_id} {short description}" \
     --label "type:spec" --label "status:planned" --label "version:{version}" \
     --body-file /tmp/issue_body.md
   ```
   Where `/tmp/issue_body.md` contains:
   ```markdown
   ---
   spec_id: sX.Y.ZZ
   version: vX.Y
   status: planned
   depends_on: []
   ---

   Identified during PR #{pr_number} review (feedback from @{author}).

   {context from the review comment}

   ## References

   - Source PR: #{pr_number}
   ```
3. If the new issue depends on the current spec, add a `blockedBy` relationship via GraphQL (see `tanren/product/github-conventions.md` → Dependency Relationships).
4. Post a reply comment referencing the new issue:
   ```
   Thanks for flagging this! It's outside the scope of this PR but I've tracked it
   as #{issue_number} for a future spec.
   ```

### Step 7: Update Audit Log

Append an entry to audit-log.md:

```
- **Feedback** (round N): {count} items — {valid_count} actionable, {addressed_count} addressed, {invalid_count} invalid, {deferred_count} out-of-scope
```

### Step 8: Commit

Commit all changes:

```
git add plan.md signposts.md audit-log.md
git commit -m "Handle feedback round N: {brief summary}"
```

Only include files that were actually modified. Push to the PR branch:

```
git push
```

### Step 9: Advise Next Steps

Tell the user what to do next:

- If fix items were added: "Re-run the orchestrator to address the feedback. Then update the PR."
- If only replies were posted: "Feedback handled. The PR is ready for re-review."
- If out-of-scope issues were created: "Deferred items tracked in: [issue URLs]"
- If style patterns emerged: "Consider running `/discover-standards` to evaluate whether [pattern] should become a standard."

### Step 10: Exit

Print one of these exit signals (machine-readable):

- `handle-feedback-status: resolved` — all feedback handled, replies posted
- `handle-feedback-status: tasks-added` — fix items added, orchestrator needs to re-run
- `handle-feedback-status: no-feedback` — no comments found on the PR
- `handle-feedback-status: error` — prerequisites missing or unrecoverable issue

## Does NOT

- Fix code itself (adds tasks for do-task)
- Merge the PR
- Modify spec.md
- Run tests or verification gates
- Make triage decisions autonomously — the user approves everything
- Post replies without user approval

## Workflow

```
walk-spec → PR created → reviewers comment / CI runs → handle-feedback → [if tasks added: orchestrator → walk-spec] → merge
```

This command closes the feedback loop between PR reviewers and the spec implementation workflow.
