# Installing Tanren Commands

`tanren-cli install` renders the shared command source under `commands/`
into per-agent-framework destinations, writes MCP server registration for each
framework, and seeds standards baselines when configured.

Canonical specs:
[architecture/install-targets.md](../architecture/install-targets.md)
(drivers, formats, merge policy),
[architecture/agent-tool-surface.md](../architecture/agent-tool-surface.md)
(MCP tool contract).

---

## Runtime contract (acceptance path)

Phase 0 acceptance uses installed binaries only:

```bash
scripts/runtime/install-runtime.sh
scripts/runtime/verify-installed-runtime.sh
```

Required binaries:
- `tanren-cli`
- `tanren-mcp`

No repo-local `cargo run -p tanren-cli` path is accepted in proof/acceptance
flows.

## Quick start

```bash
tanren-cli install             # full install into all configured targets
tanren-cli install --dry-run   # show what would be written, write nothing
tanren-cli install --strict    # fail on warnings (CI-safe drift gate)
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
    security:
      capability_issuer: tanren-phase0
      capability_audience: tanren-mcp
      capability_public_key_file: .tanren/mcp-capability-public-key.pem
      capability_private_key_file: .tanren/mcp-capability-private-key.pem
      capability_max_ttl_secs: 900
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

Hook contract for this repo:

- `task_verification_hook` should run fast per-task static gates (format/lint/compile/size/policy), no workspace test matrix.
- `spec_verification_hook` should run full strict-spec validation (all behavior gates + CI-only checks).

Installer-generated MCP config always includes `TANREN_CONFIG` plus security
inputs consumed by `tanren-mcp` startup (`TANREN_MCP_CAPABILITY_ISSUER`,
`TANREN_MCP_CAPABILITY_AUDIENCE`, `TANREN_MCP_CAPABILITY_PUBLIC_KEY_FILE`,
`TANREN_MCP_CAPABILITY_MAX_TTL_SECS`).
`TANREN_MCP_CAPABILITY_ENVELOPE` is minted dynamically at runtime and injected
per phase invocation.

Plus `tanren/rubric.yml` for pillar customization (see
[architecture/audit-rubric.md](../architecture/audit-rubric.md)).

## Template variables

Commands use `{{DOUBLE_BRACE_UPPER}}` placeholders filled at install time.
Full taxonomy is in
[architecture/install-targets.md](../architecture/install-targets.md).
Unknown variables are hard errors; undeclared-but-used or
declared-but-unused variables are hard errors.

## Per-target merge policy

| Policy | Behavior | Used for |
|---|---|---|
| `destructive` | Overwrite on reinstall | Commands (Tanren is opinionated about workflow) |
| `preserve_existing` | Never overwrite; create missing only | Standards baselines |
| `preserve_other_keys` | Merge only Tanren-owned sub-keys into existing JSON/TOML | MCP config files that also carry other tools' entries |

## Customization workflow

1. Fork `commands/` under source control.
2. Point `methodology.source.path` at your fork.
3. Re-run `tanren-cli install`.

Do not hand-edit rendered files in `.claude/commands/`, `.codex/skills/`, or
`.opencode/commands/` because install is destructive for those targets.
Standards files are preserve-existing and remain user-owned.

## Self-hosting

The Tanren repo itself is installed via `tanren-cli install`.
A convention-only `just install-commands` recipe regenerates rendered outputs;
`just install-commands-check` runs under `just ci` as a drift gate.

## See also

- [agent-tool-surface.md](../architecture/agent-tool-surface.md)
- [orchestration-flow.md](../architecture/orchestration-flow.md)
- [../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md](../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md)
