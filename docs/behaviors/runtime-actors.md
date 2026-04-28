# Runtime Actors

Canonical list of internal system actors referenced by behavior files through
the optional `runtime_actors:` frontmatter field.

Runtime actors are not product personas. They are Tanren-controlled components
or execution subjects whose public contracts matter because users, integrations,
or operators rely on them. Use runtime actors only when the behavior describes a
durable runtime/protocol obligation, not as a substitute for a user-facing
persona.

Runtime actor IDs should stay broad. Assignment phase, intent, scope,
capabilities, harness, and environment determine what a runtime actor is doing
for a given dispatch. Do not create separate actor IDs for individual phases
such as `do-task`, `audit-task`, or `investigate`; those are assignment
properties, not actor identities.

## `agent-worker`

An automated execution actor operating under Tanren's control to perform
assigned work and report evidence through supported interfaces. Covers:

- Code harness sessions such as Codex, Claude Code, or OpenCode executing
  phases
- Worker processes reporting runtime outcomes
- Agent sessions surfacing blockers, findings, patches, or evidence

Cares about:

- Clear assignment, tool, evidence, and approval contracts
- Scope-limited credentials and environment access
- Durable reporting of outcomes, failures, and blockers
- Recoverable execution that does not mutate Tanren state outside the public
  contract
