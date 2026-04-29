---
name: plan-product
description: Tanren methodology command `plan-product`
role: meta
orchestration_loop: false
autonomy: interactive
declared_variables: []
declared_tools: []
required_capabilities: []
produces_evidence:
- product brief / vision artifact
- personas artifact
- concepts artifact
- high-level README or overview updates when approved
---

# plan-product

## Temporary Status

This is a temporary Tanren-method bootstrap command. It writes direct
planning artifacts because native product-method schemas, tools, and
project-state events do not exist yet. Prefer structured frontmatter,
stable headings, explicit decisions, and small approved edits so these
artifacts can later migrate into typed Tanren storage.

This command is for any repository adopting the Tanren method. When it
is used in the Tanren repository, use Tanren's local docs as the
configured artifacts. Do not assume every repository has the same file
layout.

## Purpose

Establish and maintain product intent: what the product is, who it
serves, why it matters, which constraints and non-goals apply, what
success looks like, and which assumptions or open decisions still need
human judgment.

## Inputs

- Existing product brief, vision, motivation, or README material.
- Persona, customer, user, or actor documents.
- Concept, glossary, or domain-language documents.
- High-level architecture or product overview documents.
- Existing code, tests, and docs when adopting Tanren in an existing
  repository.
- User-provided goals, constraints, non-goals, risks, and open
  questions.

## Editable Artifacts

Use the repository's configured product-method artifact locations. If
none are configured, infer the conventional locations and confirm them
with the user before editing.

This command may create or revise:

- product brief / vision;
- motivations and success signals;
- personas and target users;
- concepts and domain terminology;
- non-goals, constraints, and assumptions;
- high-level product or architecture overview;
- README product framing when the README is the public product entry.

README edits are limited to product identity, audience, method,
positioning, and source-of-truth links unless the user explicitly asks
to revise implementation, installation, API, or CLI sections.

## Temporary Artifact Formats

Prefer this product brief shape:

```markdown
---
schema: tanren.product_brief.v0
status: draft | accepted
updated_at: YYYY-MM-DD
---

# Product Brief

## Product Identity
## Target Users
## Problems
## Motivations
## Non-Goals
## Constraints
## Success Signals
## Core Method
## Open Questions
## Change Log
```

Prefer this persona shape:

```markdown
---
schema: tanren.personas.v0
status: draft | accepted
updated_at: YYYY-MM-DD
---

# Personas

## Persona: <id>
- Name:
- Description:
- Goals:
- Pain points:
- Contexts:
```

Prefer this concept shape:

```markdown
---
schema: tanren.concepts.v0
status: draft | accepted
updated_at: YYYY-MM-DD
---

# Concepts

## <term>
Definition...
```

## Responsibilities

1. Identify the repository's product-method artifact roots and ask the
   user to confirm them if ambiguous.
2. Read the current product, persona, concept, README, and overview
   context before asking broad questions.
3. Reconstruct the current product intent from existing evidence when
   working in an existing repository.
4. Ask targeted questions only where the current artifacts are
   missing, contradictory, stale, or under-specified.
5. Propose a concise edit plan before changing files.
6. After user approval, create or revise the relevant planning
   artifacts using stable headings and structured frontmatter.
7. Preserve unresolved decisions in an open-questions section rather
   than hiding uncertainty.
8. Summarize changed product assumptions, open decisions, and likely
   follow-up behavior work.

## Out of Scope

- Creating or revising behavior files. Use `identify-behaviors`.
- Creating or revising roadmap DAG nodes. Use `craft-roadmap`.
- Asserting implementation or behavior evidence.
- Dispatching specs, creating tasks, opening pull requests, or
  mutating orchestration state.
- Rewriting implementation details unless explicitly requested by the
  user as part of the product-planning pass.
