# Walk Spec

The human validation checkpoint. The agent does the inspection work — reads the spec, reads the audit, reads the code — then walks the user through an interactive demo of the feature before submitting a PR.

**Suggested model:** Strong communicator (e.g., Opus via TUI). Needs excellent interactive skills for the demo walkthrough.

## Important Guidelines

- Human-in-the-loop — the user validates; the agent investigates and presents
- Do NOT ask the user generic yes/no checklist questions. Instead, read the code yourself, summarize what you found, and ask the user to confirm specific behaviors.
- Always use AskUserQuestion tool when asking the user anything. In TUI: use AskUserQuestion. In Discord/NanoClaw: use send_message with numbered options and wait for reply.
- If the demo fails during the walkthrough, stop and discuss with the user — do not silently fix

## Prerequisites

1. Spec implementation complete (all tasks checked in plan.md)
2. Latest audit pass (`status: pass` in audit.md)
3. Demo passed (results in demo.md)
4. Working tree clean or changes committed

If prerequisites aren't met, explain what's missing and ask the user how to proceed.

## Process

### Step 1: Resolve the Spec

Preferred inputs: issue number/URL or `spec_id`.

Resolution order:
1. Use issue from user input (via `gh issue view`).
2. If missing, read `plan.md` for `spec_id` and `issue:` metadata.
3. If still ambiguous, list spec folders and ask the user.

### Step 2: Gather Context (Autonomous)

Read all of these yourself — do not ask the user for summaries:

- **spec.md** — acceptance criteria, non-negotiables, goals
- **plan.md** — tasks and completion status
- **standards.md** — which standards apply
- **demo.md** — demo plan and latest results
- **signposts.md** — known issues, workarounds, dead ends
- **audit.md** — rubric scores, non-negotiable compliance, action items
- **audit-log.md** — full history of task audits, demo runs, spec audits
- **Implementation files** referenced in the plan and audit (follow file:line citations)

### Step 3: Run Verification Gate

Run the project's full verification gate to confirm everything still passes:

```
make all
```

If it fails, stop and report the failure — do not proceed to the demo walkthrough.

### Step 4: Implementation Summary

Present the user with a concise summary of what was implemented:

- **Key changes** — what was built, with file:line references to the most important code
- **Acceptance criteria** — how each criterion was met (cite evidence, not just checkmarks)
- **Non-negotiable compliance** — explicit pass/fail per item from audit.md
- **Audit scores** — rubric scores from audit.md
- **Demo status** — latest demo result

If there are deferred items (from audit.md), surface them and confirm the user accepts them before continuing:

```
The audit deferred these items to future specs:
- [Item] → [GitHub issue URL]
Are you okay proceeding with these deferred?
```

### Step 5: Interactive Demo Walkthrough

Read the **## Steps** section from demo.md. Present it to the user as a feature walkthrough:

1. **Set the stage** — explain what the feature does in plain terms, as if the user hasn't read the spec. Use the narrative intro from demo.md.

2. **For each step:**
   - Explain what you're about to do and why
   - Execute it (or give the user exact instructions for steps that require their interaction)
   - Show the result
   - Briefly explain what the result proves about the feature
   - Confirm before moving on to the next step

3. **If a demo step fails:**
   - Stop immediately
   - Investigate the failure (read logs, errors, code)
   - Present findings to the user
   - Do NOT fix code silently — discuss with the user whether to:
     - Fix now (exit walk-spec, go back through do-task/audit loop)
     - Accept as-is with a note
     - Abort the PR

The demo should already pass — do-task and audit-spec have both verified it. This walkthrough is about the user seeing it work with their own eyes.

### Step 6: Prepare PR

1. Ensure branch is up to date.
2. Create a PR using `gh pr create` with:
   - Title: `{spec_id} {short title}`
   - Body: summary of changes, link to spec issue, audit scores, non-negotiable compliance
3. Use this format for the PR body:

```markdown
## Summary
- [Key change 1]
- [Key change 2]
- [Key change 3]

## Spec
- Issue: #{issue_number}
- Audit: {status} ({rubric scores})
- Non-negotiables: all passed
- Demo: passed

## Deferred
- [Item] → #{deferred_issue_number}
```

### Step 7: Update Roadmap

If the spec has a corresponding entry in `tanren/product/roadmap.md`, mark it as complete (prefix with ✅). Commit this change on the branch.

### Step 8: Evaluate Standards Evolution

Skim signposts.md and audit-log.md for patterns that might warrant new or revised standards:

- Recurring issues across tasks
- Non-obvious workarounds that others would hit
- Patterns that emerged during implementation

If anything stands out, recommend the user run `/discover-standards` as a follow-up and briefly explain why. Don't embed the standards discovery logic — just suggest it.

### Step 9: Final Push and Report

1. Push the branch.
2. Update the existing "Spec Progress" comment on the issue with the PR link (do not post new comments).
3. Report to the user:
   - PR URL
   - Any follow-ups or deferred items
   - Whether `/discover-standards` is suggested and why

## Does NOT

- Implement or fix code (if demo fails, discuss with user)
- Run audit-spec
- Create branches (branch already exists from shape-spec)
- Make implementation decisions without user input

## Success Criteria

User has validated the demo, PR is submitted, and roadmap is updated.

## Workflow

```
shape-spec → [orchestrator: do-task ↔ audit-task loop → run-demo → audit-spec] → walk-spec → PR
```

Next step after this command: monitor PR checks and merge when ready.
