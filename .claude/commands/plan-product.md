---
name: plan-product
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables: []
declared_tools: []
required_capabilities: []
produces_evidence:
- docs/product/vision.md
- docs/product/personas.md
- docs/product/concepts.md
- README product framing when approved
---

# plan-product

## Temporary Status

This is a temporary Tanren-method bootstrap command. It writes markdown
projections directly because native product-planning schemas, typed tools, and
project-method events do not exist yet. Prefer structured frontmatter, stable
headings, explicit decisions, and small approved edits so these artifacts can
later migrate into typed Tanren storage.

This command is for any repository adopting the Tanren method. Use the
repository's configured product artifact paths; if none are configured, use the
conventional `docs/product/` paths.

## Purpose

Establish and maintain product intent: what the product is, who it serves, why
it matters, which constraints and non-goals apply, what success looks like, and
which assumptions or open decisions still need human judgment.

## Inputs

- Existing product brief, vision, motivation, or README material.
- Persona, customer, user, or actor documents.
- Concept, glossary, or domain-language documents.
- Existing code, tests, and docs when adopting Tanren in an existing
  repository.
- User-provided goals, constraints, non-goals, risks, and open questions.

## Editable Artifacts

This command owns product-planning projections:

- `docs/product/vision.md`
- `docs/product/personas.md`
- `docs/product/concepts.md`
- product-facing sections of `README.md` when the README is the public product
  entry

README edits are limited to product identity, audience, method, positioning,
and source-of-truth links unless the user explicitly asks to revise
implementation, installation, API, or CLI sections.

## Temporary Artifact Formats

Prefer product artifacts with frontmatter:

```yaml
---
schema: tanren.product_brief.v0
status: draft | accepted
owner_command: plan-product
updated_at: YYYY-MM-DD
---
```

Use stable headings for product identity, target users, problems, motivations,
non-goals, constraints, success signals, core method, open questions, and change
history.

## Responsibilities

1. Identify product artifact roots and ask the user to confirm them only if
   ambiguous.
2. Read current product, persona, concept, README, and overview context before
   asking broad questions.
3. Reconstruct current product intent from existing evidence when working in an
   existing repository.
4. Ask targeted questions where artifacts are missing, contradictory, stale, or
   under-specified.
5. Propose a concise edit plan before changing files.
6. After user approval, create or revise product projections using stable
   headings and structured frontmatter.
7. Preserve unresolved decisions in an open-questions section rather than
   hiding uncertainty.
8. Summarize changed product assumptions, open decisions, and likely follow-up
   behavior work.

## Out of Scope

- Creating or revising behavior files. Use `identify-behaviors`.
- Choosing implementation architecture. Use `architect-system`.
- Assessing current implementation state. Use `assess-implementation`.
- Creating or revising roadmap DAG nodes. Use `craft-roadmap`.
- Dispatching specs, creating tasks, opening pull requests, or mutating
  orchestration state.
