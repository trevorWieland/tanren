# Lane 0.5 Audit — Methodology Boundary and Self-Hosting

Audit the documentation and planned command-boundary changes for the
methodology lane.

## What to Audit

### 1. Boundary Clarity

Check that the docs make a clean distinction between:

- Tanren-code workflow responsibilities
- Tanren-markdown agent-instruction responsibilities

Flag any place where ownership is still ambiguous.

### 2. Canon Consistency

Cross-check:

- `docs/rewrite/HLD.md`
- `docs/rewrite/DESIGN_PRINCIPLES.md`
- `docs/rewrite/ROADMAP.md`
- `docs/rewrite/CRATE_GUIDE.md`
- `docs/rewrite/METHODOLOGY_BOUNDARY.md`
- `docs/methodology/system.md`
- `docs/architecture/phase-taxonomy.md`

These docs should describe the same boundary and not contradict one another.

### 3. Command Refactor Coverage

Audit the lane 0.5 docs for whether they fully specify the required shared
command cleanup:

- literal verification commands
- issue-tracker shell commands
- branch creation / checkout steps
- commit / push / PR steps
- prompts discovering the next workflow target for themselves

The audit should verify that lane 0.5 has clear ownership for removing those
behaviors from the shared command sources. It does not require those edits to
have been applied in this planning/docs pass.

### 4. Lane Separation

Check that:

- lane 0.4 remains Rust dispatch CRUD only
- lane 0.5 owns methodology boundary / self-hosting docs

## Approval Criteria

Approve only if all are true:

1. The boundary is explicit and internally consistent.
2. Lane 0.5 clearly specifies the shared-command refactor needed to remove
   literal issue/gate/SCM workflow steps.
3. Lane 0.4 and lane 0.5 scopes are cleanly separated.
4. Manual self-hosting before Phase 1 is documented as the near-term target.
