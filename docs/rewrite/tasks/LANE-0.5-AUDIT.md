# Lane 0.5 Audit — Methodology Boundary, Typed State, Tool Surface, Multi-Agent Install, Self-Hosting

Audit the Lane 0.5 execution across documentation, typed Rust domain,
tool surface, installer, and self-hosting proof.

See also:
[LANE-0.5-BRIEF.md](LANE-0.5-BRIEF.md),
[LANE-0.5-METHODOLOGY.md](LANE-0.5-METHODOLOGY.md),
[LANE-0.5-DESIGN-NOTES.md](LANE-0.5-DESIGN-NOTES.md).

---

## 1. Mechanical sweeps (must return zero hits)

Run these greps on the lane's final state. Any hit is a hard audit
fail unless it occurs inside documentation showing what NOT to do
(clearly labeled).

```
rg -n '^\s*(gh|git|make|just ci|cargo|docker)\b' commands/
rg -n '\.agent-status' commands/
rg -n 'find the next|select the next|choose a gate|create the issue' commands/
rg -n 'edit plan\.md|update plan\.md|check off' commands/
rg -n 'TODO|FIXME' crates/tanren-domain/src/methodology/
```

## 2. Boundary clarity

Check every concern listed in the operational-ownership table in
[METHODOLOGY_BOUNDARY.md](../METHODOLOGY_BOUNDARY.md) is owned
unambiguously by `tanren-code`. Flag any place where ownership reads
as "either" or "depends".

## 3. Canon consistency

Cross-check these documents tell the same boundary / tool-surface /
task-lifecycle story:

- [docs/rewrite/HLD.md](../HLD.md) §6
- [docs/rewrite/DESIGN_PRINCIPLES.md](../DESIGN_PRINCIPLES.md)
  (principles 11, 12, 13)
- [docs/rewrite/ROADMAP.md](../ROADMAP.md) Phase 0 exit criteria
- [docs/rewrite/CRATE_GUIDE.md](../CRATE_GUIDE.md) linking rules §7
- [docs/rewrite/METHODOLOGY_BOUNDARY.md](../METHODOLOGY_BOUNDARY.md)
- [docs/methodology/system.md](../../methodology/system.md)
- [docs/methodology/commands-install.md](../../methodology/commands-install.md)
- [docs/architecture/phase-taxonomy.md](../../architecture/phase-taxonomy.md)
- [docs/architecture/orchestration-flow.md](../../architecture/orchestration-flow.md)
- [docs/architecture/agent-tool-surface.md](../../architecture/agent-tool-surface.md)
- [docs/architecture/evidence-schemas.md](../../architecture/evidence-schemas.md)
- [docs/architecture/audit-rubric.md](../../architecture/audit-rubric.md)
- [docs/architecture/adherence.md](../../architecture/adherence.md)
- [docs/architecture/install-targets.md](../../architecture/install-targets.md)

## 4. Typed domain audit

`tanren-domain::methodology`:
- Task state machine enforces `Complete` terminal; illegal transitions
  return typed errors.
- `Abandoned` requires replacements or explicit user discard.
- Property tests cover all guard orderings and out-of-order arrival.
- Every `DomainEvent` variant added by Lane 0.5 round-trips through
  serde with stable JSON.
- Evidence frontmatter schemas round-trip (parse → render → parse).

## 5. Tool-surface audit

`tanren-contract::methodology`:
- Every tool in the catalog (agent-tool-surface.md) has a typed
  schema.
- Every schema has a `schema_version` field.

`tanren-app-services::methodology::service`:
- Every tool validates input before side effects.
- Invalid input returns typed `ToolError { field_path, expected,
  actual, remediation }`.
- Phase capability enforcement rejects out-of-scope calls with
  `CapabilityDenied`.
- `escalate_to_blocker` callable only from `investigate`.
- `post_reply_directive` callable only from `handle-feedback`.
- `create_issue` callable only from `triage-audits` and
  `handle-feedback`.
- Rubric invariants enforced at `record_rubric_score` call time (see
  audit-rubric.md §2.2).
- Tools are idempotent on re-call with identical content.

## 6. Transport parity audit

`tanren-mcp` and `tanren-cli`:
- Both transports call the same service methods.
- For any valid tool input, both produce identical events.
- Event log contents are identical between transports.
- MCP server writes to stderr only (stdout reserved for JSON-RPC).
- Evidence must be traceable to executable tests:
  - valid mutation parity proof:
    `bin/tanren-mcp/tests/transport_parity.rs` (includes
    `bin/tanren-cli/tests/cli_mcp_parity_impl.inc`)
  - invalid-input no-side-effect parity proof:
    `bin/tanren-mcp/tests/transport_parity.rs`

## 7. Installer audit

`tanren-cli install`:
- Dry-run produces exact diff without writes.
- Apply is idempotent (re-run produces no changes).
- `--strict --dry-run` fails with exit 3 when drift exists.
- Unknown template variables fail rendering with file:line info.
- Claude Code, Codex Skills, OpenCode rendered content is semantically
  identical (hash of canonicalized form equal).
- Standards baselines with `preserve_existing` never overwrite.
- MCP configs with `preserve_other_keys` only touch tanren-owned
  sub-keys.
- Codex Skills output is one directory per command with `SKILL.md`.
- OpenCode output has prompt body in `template` frontmatter field.

## 8. Self-hosting audit

In the tanren repo:
- `.claude/commands/`, `.codex/skills/`, `.opencode/commands/`
  committed as rendered artifacts.
- `.mcp.json`, `.codex/config.toml`, `opencode.json` reference
  `tanren-mcp`.
- `just install-commands-check` runs clean under `just ci`.
- No `install-commands-check` behavior is prescribed to downstream
  consumers.

## 9. Lane separation audit

- Lane 0.4: Rust dispatch CRUD only. No methodology work.
- Lane 0.5: methodology + installer + self-hosting. No harness /
  environment / runtime implementation (those are Phase 1+).

## 10. Approval criteria

Approve only if:
1. All mechanical sweeps return zero hits.
2. Boundary is explicit and internally consistent across all 13 canon
   docs.
3. Typed domain enforces monotonicity and guard composition by
   property test.
4. Tool surface enforces schema and capability at call time; all
   transport pairs produce identical events.
5. Installer produces multi-target parity; standards preserve;
   MCP configs merge non-destructively.
6. Self-hosting in the tanren repo compiles, renders, and `just ci`
   (including drift check) passes.
7. Lane 0.4 / 0.5 scopes are disjoint.
8. Manual self-hosting (7-step sequence) is documented as the
   pre-Phase-1 target.
