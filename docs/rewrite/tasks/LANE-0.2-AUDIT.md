# Lane 0.2 — Domain Model — Audit Brief

## Role

You are auditing the `tanren-domain` crate implementation. Your job is not to
rubber-stamp — it is to ensure this foundation is correct, durable, and won't
create costly problems downstream. Every other crate in the workspace depends on
this one. Mistakes here propagate everywhere.

## Required Reading

Before auditing, read these fully:
1. `docs/rewrite/tasks/LANE-0.2-DOMAIN.md` — the full spec
2. `docs/rewrite/DESIGN_PRINCIPLES.md` — the 10 decision rules
3. `docs/rewrite/CRATE_GUIDE.md` — linking rules and crate boundaries
4. `CLAUDE.md` — workspace quality conventions

## Audit Dimensions

### 1. Spec Fidelity — Does It Solve the Right Problem?

The spec describes *what* to build, but the deeper intent is in the planning
docs. Check whether the implementation honors the *spirit*, not just the letter:

- **Contract-first**: Are these types suitable as the single source of truth from
  which CLI, API, MCP, and TUI schemas can be derived? Or do they leak
  implementation details that would force interface-specific workarounds?
- **Planner-native**: Do the command/event types support graph-based planning
  (dispatch graphs with dependencies), not just flat step sequences? The Python
  system was step-centric — the rewrite must not repeat this.
- **Policy as first-class**: Are policy decisions modeled as typed domain events
  with audit trails? Or are they afterthoughts that will need bolted on later?
- **Lease lifecycle**: The unified execution lease model (Requested → Provisioning
  → Ready → Running → Idle → Draining → Released) is new in 2.0. Is it
  properly represented with the same rigor as dispatch/step lifecycles?
- **Multi-tenant from the start**: Do ID types and event structures support
  user/team/org attribution without requiring schema changes later?

### 2. Correctness

- **State machine completeness**: Every valid transition is allowed. Every invalid
  transition is rejected. No states are unreachable. Terminal states have no
  outgoing transitions. Verify with exhaustive match coverage.
- **Guard logic**: Test that guards reject exactly the right cases. Reference the
  Python guard bugs that were fixed (TOCTOU races, duplicate teardown edge cases)
  and verify the Rust version doesn't reintroduce them.
- **Event completeness**: Can the full lifecycle of a dispatch (create → provision
  → execute → gate → audit → teardown → complete/fail/cancel) be reconstructed
  from events alone? Are there lifecycle transitions that don't emit events?
- **Serde stability**: Would adding a new event variant or enum value break
  deserialization of existing data? Check for `#[serde(rename_all)]`,
  `#[serde(tag)]`, default handling, and unknown variant behavior.

### 3. Performance

- **Clone cost**: Domain types will be cloned frequently (into events, across
  async boundaries). Are there any types that are unexpectedly expensive to clone?
  Large nested structures, unnecessary `Vec`/`String` copies?
- **Serialization cost**: Events are serialized on every append. Are payload types
  lean, or do they carry redundant data that inflates every write?
- **Enum size**: Rust enums are sized to their largest variant. If `DomainEvent`
  has one huge variant and many small ones, every event pays the memory cost of
  the largest. Check for this and consider `Box`ing large variants.

### 4. Security

- **Secret leakage**: Do any domain types carry secret values (API keys, tokens,
  credentials) in the clear? Secret fields should use `secrecy::Secret<T>` or be
  excluded from Debug/Serialize implementations.
- **Debug safety**: Does `#[derive(Debug)]` on any type print sensitive data?
  Check Dispatch, any auth-related types, and anything that carries environment
  variables or credentials.
- **Input validation**: Do commands validate inputs (e.g., timeout > 0, non-empty
  project name) at construction time, or do they defer validation to callers?
  Prefer validation at construction — invalid domain objects shouldn't exist.

### 5. Maintainability

- **File size**: No file over 500 lines, no function over 100 lines.
- **Module cohesion**: Each module should have one clear responsibility. If
  `events.rs` is doing event definition AND serialization helpers AND filtering
  logic, it should be split.
- **Naming**: Types should be unambiguous without their module path. `Status` is
  bad; `DispatchStatus` is good.
- **Documentation**: Every public type and function has a doc comment. Doc
  comments explain *why*, not just *what*.

### 6. Scalability and Extensibility

- **Adding a new harness**: If someone adds a new CLI tool (e.g., `Cursor`), how
  many files need to change? Ideally just the `Cli` enum and `cli_to_lane()`.
- **Adding a new event type**: Can a new event variant be added without breaking
  existing deserialization? Check backward compatibility strategy.
- **Adding a new step type**: If a new step type beyond provision/execute/teardown
  is needed, does the type system accommodate it cleanly?
- **Cross-crate impact**: If a type in domain changes, how many downstream crates
  break? Types used across many crates should be especially stable.

## Audit Process

1. Read the full spec and planning docs listed above
2. Read every `.rs` file in `crates/tanren-domain/src/`
3. Run `just ci` — verify it passes
4. Run `cargo test -p tanren-domain` — verify test quality (not just green)
5. For each dimension above, write specific findings
6. Classify each finding: **blocker** (must fix), **issue** (should fix),
   **suggestion** (consider for later)
7. If you find blockers, describe the fix concretely — don't just flag the problem

## Output Format

Deliver findings as a structured list grouped by dimension. For each finding:

```
### [BLOCKER|ISSUE|SUGGESTION] — Short title

**Dimension:** Correctness / Performance / Security / etc.
**Location:** crates/tanren-domain/src/status.rs:42
**Finding:** Description of what's wrong and why it matters.
**Fix:** Concrete fix or direction.
```
