# Installing Tanren Commands

`tanren install` renders the shared command source under `commands/`
into per-agent-framework destinations, writes the MCP server
registration for each framework, and optionally seeds repo-specific
standards baselines.

Canonical specs:
[architecture/install-targets.md](../architecture/install-targets.md)
(drivers, formats, paths),
[architecture/agent-tool-surface.md](../architecture/agent-tool-surface.md)
(what the MCP config points to).

---

## Quick start

```bash
tanren install             # full install into all configured targets
tanren install --dry-run   # show what would be written, write nothing
tanren install --strict    # fail on warnings (CI-safe drift gate)
```

Exit codes:
- `0` success (or dry-run no drift)
- `1` config/render error
- `2` write error
- `3` dry-run detected pending changes
- `4` validation error (missing required metadata, etc.)

## Configuration

Add a `methodology:` section to `tanren.yml`:

```yaml
methodology:
  task_complete_requires: [gate_checked, audited, adherent]

  source:
    path: commands

  install_targets:
    - path: .claude/commands
      format: claude-code
      binding: mcp
      merge_policy: destructive

    - path: .codex/skills
      format: codex-skills
      binding: mcp
      merge_policy: destructive

    - path: .opencode/commands
      format: opencode
      binding: mcp
      merge_policy: destructive

    - path: tanren/standards
      format: standards-baseline
      binding: none
      merge_policy: preserve_existing

  mcp:
    transport: stdio
    enabled: true
    also_write_configs:
      - path: .mcp.json
        format: claude-mcp-json
        merge_policy: preserve_other_keys
      - path: .codex/config.toml
        format: codex-config-toml
        merge_policy: preserve_other_keys
      - path: opencode.json
        format: opencode-json
        merge_policy: preserve_other_keys

  variables:
    task_verification_hook: "just check"
    spec_verification_hook: "just ci"
    issue_provider: GitHub
    project_language: rust
```

Plus `tanren/rubric.yml` for pillar customization (see
[architecture/audit-rubric.md](../architecture/audit-rubric.md)).

## Template variables

Commands use `{{DOUBLE_BRACE_UPPER}}` placeholders filled at install
time. Full taxonomy in
[architecture/install-targets.md](../architecture/install-targets.md).
Unknown variables are hard errors; undeclared-but-used or
declared-but-unused variables are hard errors too.

## Per-target merge policy

| Policy | Behavior | Used for |
|---|---|---|
| `destructive` | Overwrite on reinstall | Commands (tanren is opinionated about workflow) |
| `preserve_existing` | Never overwrite; create missing only | Standards baselines (repo tailors to its own needs) |
| `preserve_other_keys` | Merge only tanren-owned sub-keys into existing JSON/TOML | MCP config files that also carry other tools' entries |

## Customization workflow

1. Fork `commands/` under source control.
2. Point `methodology.source.path` at your fork.
3. Re-run `tanren install`.

Do NOT hand-edit the rendered files in `.claude/commands/`,
`.codex/skills/`, or `.opencode/commands/` — they are destructively
overwritten on reinstall. Standards files are different: hand-edit
them freely, they persist.

## Self-hosting

The tanren repo is itself installed via `tanren install`. A
convention-only `just install-commands` recipe in the tanren repo's
justfile regenerates all three rendered directories; `just
install-commands-check` runs under `just ci` as a drift gate. These
recipes are **tanren-specific dogfooding**, not prescribed to
downstream adopters.

## See also

- [agent-tool-surface.md](../architecture/agent-tool-surface.md) —
  what the installed MCP server exposes
- [orchestration-flow.md](../architecture/orchestration-flow.md) —
  runtime behavior of the installed commands
- [../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md](../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md)
  — design rationale
