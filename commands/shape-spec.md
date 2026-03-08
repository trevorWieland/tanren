# Shape Spec

Plan the work with the user. Produce all spec artifacts and push. This is the only command where the user and agent collaborate interactively on what to build.

**Suggested model:** Strong planner with good communication skills (e.g., Opus via TUI).

## Important Guidelines

- Always use AskUserQuestion tool when asking the user anything. In TUI: use AskUserQuestion. In Discord/NanoClaw: use send_message with numbered options and wait for reply.
- Offer suggestions — present options the user can confirm or adjust
- Keep it lightweight — this is shaping, not exhaustive documentation
- Prefer GitHub issues as the source of truth for spec id, title, status, and dependencies
- spec.md is the immutable contract — get it right here, because no automated command will change it

## Prerequisites

Steps 1–11 are discussion only — do NOT create branches, write files, or run git commands until Step 12.

## Process

### Step 1: Resolve the Spec Issue

If the user provided an issue number, URL, or spec id:

1. Fetch the issue via `gh issue view` to get title, body, labels, milestone, and relationships.
2. Extract `spec_id`, `version` (from `version:*` label), and dependencies (blocked-by relationships).

If no issue was provided, use AskUserQuestion:

```
Do you want me to create a new spec issue on GitHub before shaping?

1. I have an existing issue
2. Create a new issue now
3. Pick the next best issue from the roadmap
```

If picking the next best issue:

1. Run the candidate finder script — it resolves dependency chains and shows only the earliest milestone:

```
python3 tanren/scripts/list-candidates.py
```

2. Present the candidates to the user with AskUserQuestion. Do NOT do any additional research, exploration, or codebase analysis at this point — just show the script output and let the user pick.
3. Once chosen, fetch the full issue with `gh issue view <number>`.

If using an existing issue:

1. Ask for the issue number or URL.
2. Fetch the issue body and parse YAML frontmatter (between `---` delimiters) to extract `spec_id`, `version`, `status`, and `depends_on`. If no frontmatter exists, extract `spec_id` from the title and note that the body will need frontmatter added in Step 12.
3. Query existing `blockedBy` relationships via GraphQL (see `tanren/product/github-conventions.md` → Querying blockedBy) to confirm dependency data. If GraphQL blockedBy is unavailable, verify dependencies from frontmatter `depends_on` field instead.

If creating a new issue:

1. Determine the next available `spec_id` by scanning GitHub issues for the target version (see `tanren/product/github-conventions.md` → Resolving spec_id).
2. Create the issue with YAML frontmatter body (Format A):
   ```bash
   gh issue create \
     --title "{spec_id} {title}" \
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
   depends_on: []
   ---

   {brief description from user}

   ## Acceptance Criteria

   - [ ] TBD — flesh out during shaping
   ```
3. If the spec has dependencies, add `blockedBy` relationships via GraphQL (see `tanren/product/github-conventions.md` → Dependency Relationships).
4. Use the new issue as the shaping context.

### Step 2: Clarify Scope

Use the issue title/body as the primary scope. Ask only if unclear:

```
I pulled scope from the issue. Any constraints or expected outcomes I should add?
```

### Step 3: Gather Visuals (Optional)

> For doc-only or standards-only changes, steps 3-5 are typically N/A. You may collapse them into a single confirmation: "This is a doc/standards change — skipping visuals, references, and product context. Any objections?"

```
Any visuals to reference? (mockups, screenshots, examples, or "none")
```

### Step 4: Reference Implementations (Optional)

```
Is there similar code in this codebase I should reference? (paths or "none")
```

### Step 5: Product Context (Quick Alignment)

If `tanren/product/` exists, skim key files and ask:

```
Any product goals or constraints this spec should align with?
```

### Step 6: Standards

Read `tanren/standards/index.yml` and propose relevant standards. Confirm with AskUserQuestion.

### Step 7: Non-Negotiables

Non-negotiables are the things auditors must never compromise on. They go into spec.md under **Note to Code Auditors** and are checked by every audit command in the lifecycle.

Work with the user to define these. Propose based on the scope and ask for confirmation. Good non-negotiables are:

- **Specific** — not "code should be clean" but "no dict[str, Any] in routing config"
- **Verifiable** — an auditor can check pass/fail with evidence
- **Important** — things that, if violated, mean the feature is fundamentally broken

Example:

```
Here are proposed non-negotiables for this spec:

1. No mixed output modes — the runtime must use tool output exclusively
2. All routing config must use typed Pydantic models
3. No test deletions or modifications to make audits pass

Would you adjust any of these?
```

### Step 8: Define Acceptance Criteria

Draft acceptance criteria for spec.md. These are the contract — they define when the spec is done. Each criterion should be:

- **Observable** — can be verified by reading code or running a command
- **Scoped** — tied to this spec, not general quality
- **Complete** — if all criteria pass, the feature works

Present to the user for confirmation.

### Step 9: Demo Plan

Every spec must include a demo plan in demo.md. The demo is the chance to prove the feature works interactively — not through passing tests, but by actually using the feature the way a user would.

Write the demo plan as if you're pitching the feature to someone who knows nothing about the implementation. Start with what the feature is and why it matters, then describe how you'll prove it works.

Example framing (adapt to the feature):

```
The project now works whether you use local models or a cloud provider.
In this demo, we'll prove how both work, how to switch between them,
and why this matters for reliability.

1. Run the pipeline with the default local provider — show it completes.
2. Switch the config to the cloud provider — show the same pipeline completes.
3. Show the logs confirming the correct provider routing.
```

#### 9a: Environment Assessment

Before finalizing the demo steps, assess and **verify** the execution environment with the user. The goal is to determine **upfront** which steps the autonomous `run-demo` agent can actually execute, so it doesn't have to guess later.

**Why this matters:** The `run-demo` agent runs autonomously and cannot ask questions. If the environment section is vague (e.g., "local model server running" without a URL), the agent will guess — and guess wrong. Every detail the agent would need to connect to a service must be captured here, during shaping, while the user is present.

Also probe for available agent CLIs: claude, codex, opencode, and other agent CLIs that the orchestrator will need.

Ask using AskUserQuestion:

```
For the autonomous demo runner, what's available in the execution environment?
- API keys / credentials: [e.g., API_KEY in .env]
- External services with exact URLs: [e.g., OpenRouter at https://openrouter.ai/api/v1, LM Studio at http://192.168.1.23:1234/v1]
- Hardware / resources: [e.g., 16GB RAM, no GPU]
- Special setup needed: [e.g., "run make seed first", or "none"]
```

##### Connection string rule

For every external service the demo depends on, capture the **exact connection string** (URL, port, hostname) in the Environment section. Well-known cloud APIs with standard URLs (e.g., OpenRouter, OpenAI) can use their standard URL. Local or custom services **must** include the exact URL — never just "local model server" or "localhost".

##### Probe verification rule

For every service listed in the environment, **probe it during shaping** to confirm it's reachable:

```bash
# Cloud API — check auth works
curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $API_KEY" https://openrouter.ai/api/v1/models

# Local/custom service — check it responds
curl --connect-timeout 5 http://192.168.1.23:1234/v1/models
```

Record the probe result in the Environment section. If a service is unreachable during shaping, any demo step that depends on it **must** be classified as `[SKIP]`.

##### Step classification

After environment assessment, classify each step and confirm with the user:

```
Based on the environment probes, here's how I'd classify each demo step:
1. **[RUN]** Run unit tests — no external dependencies
2. **[RUN]** Run pipeline via OpenRouter — API key verified, endpoint responds (200)
3. **[SKIP]** Run with local model — server at 192.168.1.23:1234 not reachable (connection refused)

Does this look right?
```

Classify each step as:

- **[RUN]** — The autonomous demo agent MUST execute this step. Only use this when the environment has been verified as supporting it during shaping (probes passed, keys present, services reachable). If a [RUN] step fails during `run-demo`, it's a **blocker** — something broke that was working during shaping.
- **[SKIP]** — The step cannot be executed in the current environment. Skipped entirely by `run-demo` — does not count toward pass or fail. Use when a service is unreachable, hardware is unavailable, or a dependency is missing. Must include the reason (e.g., "server unreachable during shaping").

The classification is written into demo.md and becomes binding for `run-demo`. The agent does not get to reclassify steps at execution time.

#### 9b: Finalize Demo Plan

Propose the demo and confirm with AskUserQuestion:

```
Here's my proposed demo for this feature:
[demo plan with [RUN]/[SKIP] tags]
Does this cover what matters, or would you adjust it?
```

Demo plan qualities:

- **Narrative** — opens with what the feature is and why it matters
- **Concise** — a focused proof, not an exhaustive test suite
- **Specific** — concrete actions and observable outcomes
- **Accessible** — someone unfamiliar with the implementation could follow it
- **Medium-agnostic** — CLI, TUI, web UI, config changes, API calls, whatever fits
- **Executable by default** — most steps should be [RUN]; [SKIP] requires justification (failed probe during shaping)

The demo plan is used by every subsequent command:
- `run-demo` (via orchestrator) executes [RUN] steps and skips [SKIP] steps
- `audit-spec` verifies the demo was run and passed
- `walk-spec` walks the user through the demo interactively

### Step 10: Task Plan

Structure the implementation as a checklist in plan.md. Use the demo plan to inform what tests are needed — the test suite should guarantee the demo will pass.

Task 1 must always be **Save Spec Documentation** (shape-spec completes this task during the commit step — see Step 12). The final task should be the last implementation task. Do not add a task for running the verification gate — the orchestrator handles that automatically.

Plan quality requirements:

- Each task includes concrete steps and referenced files (paths or modules)
- Include test expectations per task where relevant
- Add acceptance checks tied to user value

Think backwards from the demo: what would need to be tested to guarantee each demo step succeeds?

Present the plan to the user for confirmation.

### Step 11: Spec Folder Name

Create:

```
YYYY-MM-DD-HHMM-{spec_id}-{feature-slug}/
```

### Step 12: Create Branch, Save Spec Docs, Commit, and Push

> Before creating a branch, run `git status`. If the working tree is dirty or has uncommitted changes, STOP and inform the user. Do not stash or switch branches with a dirty tree.

1. Create the issue branch via `gh issue develop` and check it out.
2. Create the spec folder and write all spec files:
   - `spec.md` — acceptance criteria, non-negotiables, Note to Code Auditors (this is the immutable contract)
   - `plan.md` — checklist tasks, decision record, metadata
   - `demo.md` — narrative demo plan
   - `standards.md` — applicable standards list
   - `references.md` — implementation files, issues, related specs
   - Any visuals in `visuals/`
3. Check off Task 1 in plan.md: `[ ]` → `[x]` for "Save Spec Documentation" — this task is now complete.
4. Commit **only** the spec docs on the issue branch.
5. Update the **issue body** (do not post a new comment). Preserve the existing YAML frontmatter — update `status` to `in-progress` in the frontmatter, then append a "Spec Summary" section below the frontmatter that includes:
   - Spec folder path
   - Plan summary (tasks and acceptance checks)
   - Non-negotiables
   - References and standards applied

   If the issue body has no YAML frontmatter yet, prepend one (see `tanren/product/github-conventions.md` → Issue Body Format). Use the spec's metadata for `spec_id`, `version`, `depends_on`, and set `status: in-progress`.
6. Push the branch with `-u` to publish it.

### Step 13: Stop and Handoff

Do not start implementation. The orchestrator handles everything from here.

Next step: run the orchestrator script, which invokes `do-task`, `audit-task`, `run-demo`, and `audit-spec` automatically.

## Output Structure

```
tanren/specs/{YYYY-MM-DD-HHMM-spec_id-feature-slug}/
├── spec.md          # Acceptance criteria, non-negotiables (IMMUTABLE)
├── plan.md          # Checklist tasks, decision record (MUTABLE)
├── demo.md          # Demo plan (plan section immutable, results appended later)
├── standards.md     # Applicable standards list
├── references.md    # Implementation files, issues, related specs
└── visuals/         # Optional mockups, diagrams, screenshots
```

## File Format: spec.md

```markdown
spec_id: sX.Y.ZZ
issue: https://github.com/OWNER/REPO/issues/NNN
version: vX.Y

# Spec: [Title]

## Problem
[Brief statement of what's broken or missing]

## Goals
- [Goal 1]
- [Goal 2]

## Non-Goals
- [Explicit exclusion 1]
- [Explicit exclusion 2]

## Acceptance Criteria
- [ ] [Criterion 1]
- [ ] [Criterion 2]
- [ ] All tests pass including full verification gate
- [ ] Demo passes (see demo.md)

## Note to Code Auditors

Non-negotiables for this spec. Do not approve if any of these are violated:

1. **[Non-negotiable 1]** — [explanation]
2. **[Non-negotiable 2]** — [explanation]
```

## File Format: plan.md

```markdown
spec_id: sX.Y.ZZ
issue: https://github.com/OWNER/REPO/issues/NNN
version: vX.Y

# Plan: [Title]

## Decision Record
[Why this work exists — brief rationale]

## Tasks
- [ ] Task 1: Save Spec Documentation
- [ ] Task 2: [Description]
  - [Concrete step]
  - [Referenced file or module]
  - [Test expectation]
- [ ] Task 3: [Description]
  ...
- [ ] Task N: [Last implementation task]
```

## File Format: demo.md

```markdown
# Demo: [Title]

[Narrative intro — what the feature is, why it matters]

## Environment

- API keys: [key name and source, e.g., "OPENROUTER_API_KEY via .env"]
- External services: [service name, exact URL, and probe result during shaping]
  - OpenRouter API at https://openrouter.ai/api/v1 — verified (200)
  - LM Studio at http://192.168.1.23:1234/v1 — verified (200) | unreachable (connection refused)
- Setup: [any pre-demo setup, or "none"]

## Steps

1. **[RUN]** [Action] — expected: [observable outcome]
2. **[RUN]** [Action] — expected: [observable outcome]
3. **[SKIP]** [Action] — reason: [why this can't run, e.g., "server unreachable during shaping"]

## Results

(Appended by run-demo — do not write this section during shaping)
```

Step classification rules:
- **[RUN]** is the default. The autonomous demo agent MUST execute these steps. Environment was verified during shaping — if it fails, it's a blocker.
- **[SKIP]** means the step cannot run due to a documented environment limitation verified during shaping (probe failed, service unreachable). Skipped entirely — does not count toward pass or fail.
- Classifications are set during shape-spec and are binding. run-demo does not reclassify.

## Workflow

```
shape-spec → [orchestrator: do-task ↔ audit-task loop → run-demo → audit-spec] → walk-spec → PR
```

Next step after this command: run the orchestrator.
