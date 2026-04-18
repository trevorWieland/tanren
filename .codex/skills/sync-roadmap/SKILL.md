---
name: sync-roadmap
role: meta
orchestration_loop: false
autonomy: autonomous
declared_variables:
- ISSUE_PROVIDER
- ISSUE_REF_NOUN
- PRODUCT_ROOT
- READONLY_ARTIFACT_BANNER
- TASK_TOOL_BINDING
declared_tools:
- add_finding
- post_reply_directive
- report_phase_outcome
required_capabilities:
- finding.add
- feedback.reply
- phase.outcome
produces_evidence: []
---

# sync-roadmap

## Purpose

Reconcile `tanren/product/roadmap.md` with the real spec-completion
state held in the Tanren store plus the `GitHub` issue
source. Emit a structured diff of reconciling actions; Tanren-code
performs all mutations.

## Inputs (from your dispatch)

- The supplied reconciliation context: current roadmap snapshot,
  issue-source snapshot (filtered to spec-type GitHub issues),
  and the store's spec completion list.
- Divergences already pre-computed by Tanren-code.

## Responsibilities

1. Read the reconciliation context. Identify:
   - Specs in roadmap but not in the issue source (→ create issue).
   - Issues tagged as specs but missing from roadmap (→ add to
     roadmap).
   - Specs with mismatched status (closed issue but status:planned,
     etc.).
   - Dependency divergences (frontmatter `depends_on` vs issue
     `blockedBy`).
2. For each reconciling action needed, emit `add_finding` with
   severity `fix_now` or `defer`, tagged with the action shape
   (create/update/relink). Orchestrator applies the mutations.
3. If user confirmation is needed for a destructive reconciliation
   (e.g. closing a stale roadmap entry), emit
   `post_reply_directive` flagged for the operator.
4. `report_phase_outcome("complete", <summary>)`.

## Emitting results

mcp

⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT.
plan.md and progress.json are generated from the typed task store.
Postflight reverts unauthorized edits and emits an
UnauthorizedArtifactEdit event. Use the typed tool surface
(MCP or CLI) to record progress.


## Out of scope

- Calling `GitHub` shell commands directly
- Editing `roadmap.md` directly (orchestrator does, based on your
  findings)
- Creating tasks (this command is cross-spec; it creates
  reconciliation findings, not spec-scope tasks)
- Mutating dependency graphs directly
