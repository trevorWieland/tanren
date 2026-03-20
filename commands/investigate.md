# Investigate

Read-only root cause analysis for persistent failures. Diagnose the problem — do NOT fix it. Fully autonomous — no user interaction.

**Suggested model:** Capable reasoner for diagnosis. Autonomous — no user interaction.

## Important Guidelines

- **READ-ONLY: Do NOT modify any code files, tests, or configs.**
- **Do NOT commit.** Your only output is `investigation-report.json` in the spec folder.
- **Do NOT edit spec.md, plan.md, progress.json, .gitignore, or any source files.**
- You are given specific context about what failed and what was attempted.
- Focus on root cause, not symptoms.
- Be concrete — cite files, lines, and specific code when identifying causes.

## Prerequisites

1. A spec folder exists with `spec.md` and `plan.md`
2. Context is provided describing the failure (trigger, task, output)

If context is missing, exit with `investigate-status: error` and explain what's needed.

## Process

### Step 1: Load Context

Read the context provided in the dispatch. Understand:

- **What was attempted** — task title, phase that failed
- **What the error was** — failure output, gate results, test failures
- **What's been done so far** — completed tasks, code changes

Read the spec files for broader context:
- **spec.md** — what the spec is trying to achieve
- **plan.md** — task structure and current state
- **signposts.md** (if exists) — known issues, constraints, and prior resolutions

### Step 2: Investigate

Read relevant code, tests, and configuration. Trace the failure:

1. **Follow the error** — if gate output or test failures are provided, read the failing test code, then the implementation code it tests.
2. **Check for root cause categories:**
   - `code_bug` — implementation error, missing logic, wrong function call, import error
   - `spec_ambiguity` — spec is unclear or contradictory, can't determine correct behavior
   - `demo_plan_issue` — demo plan assumes something that isn't true, or steps are in wrong order
3. **Identify affected files** — list every file involved in the root cause chain.
4. **Assess confidence** — `high` if you can point to a specific line/function, `medium` if you have a strong hypothesis, `low` if unclear.

### Step 3: Write Report

Write `investigation-report.json` in the spec folder:

```json
{
  "trigger": "gate_failure_persistent|demo_failure|audit_failure|agent_blocked",
  "root_causes": [
    {
      "description": "Clear explanation of what's wrong and why",
      "confidence": "high|medium|low",
      "affected_files": ["src/module.py", "tests/test_module.py"],
      "category": "code_bug|spec_ambiguity|demo_plan_issue",
      "suggested_tasks": [
        {
          "title": "Short imperative description of fix",
          "description": "Detailed instructions with file:line references"
        }
      ]
    }
  ],
  "unrelated_failures": [
    {
      "test": "test_name",
      "reason": "Why this failure is unrelated to the current investigation"
    }
  ],
  "escalation_needed": false,
  "escalation_reason": null
}
```

Rules:
- Every root cause must have at least one `suggested_task` (even if confidence is low — suggest what to try)
- Set `escalation_needed: true` only if the issue genuinely requires human judgment (spec ambiguity, architectural decision)
- `unrelated_failures` helps the coordinator filter noise — list any test failures you found that aren't caused by the current task's changes

### Step 4: Exit

Print: `investigate-status: complete`

If you couldn't produce a meaningful report: `investigate-status: error`

## Does NOT

- Fix code or modify any source files
- Commit anything
- Edit spec.md, plan.md, progress.json, or .gitignore
- Push or touch GitHub
- Retry the failed operation

## Workflow

```
shape-spec → [orchestrator: do-task ↔ audit-task loop → run-demo → audit-spec] → walk-spec → PR
                                    ↑
                              investigate (dispatched on persistent failures)
```

The orchestrator dispatches investigate when repeated attempts to fix a failure haven't worked. Your report helps the orchestrator decide whether to add new fix tasks, halt for human input, or try a different approach.
