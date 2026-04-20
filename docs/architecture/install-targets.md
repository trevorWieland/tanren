# Install Targets

Authoritative spec for `tanren install`: how shared command sources,
MCP server configurations, and standards baselines are rendered and
written to agent-framework-specific destinations.

Companion docs:
[agent-tool-surface.md](agent-tool-surface.md)
(what MCP configs reference).

---

## 1. Principles

1. **Single source, multiple renders.** One canonical `commands/`
   source tree; the installer emits per-target rendered artifacts.
2. **Semantic parity across targets.** Content is identical; only
   format wrappers differ.
3. **Per-target merge policy.** Commands are destructive; standards
   are preserving; configs are preserve-other-keys.
4. **Destructive-on-reinstall for commands by design.** If a user
   wants to customize, they fork `commands/` and point the installer
   at their fork.
5. **Idempotent.** Running `tanren install` twice produces no disk
   change when config and source haven't changed.
6. **Dry-run friendly.** `--dry-run` shows every write without
   performing it; exit code 3 when drift exists (usable as a CI
   gate).

---

## 2. Target formats (from 2026 research)

### 2.1 Claude Code (`claude-code`)

- **Path:** `{target}/<command>.md`
- **Format:** YAML frontmatter + markdown body
- **MCP config:** `.mcp.json` (JSON)
  ```jsonc
  {
    "mcpServers": {
      "tanren": {
        "command": "tanren-mcp",
        "args": ["serve"],
        "env": { "TANREN_CONFIG": "./tanren.yml" }
      }
    }
  }
  ```
- **Merge (commands):** `destructive`
- **Merge (config):** `preserve_other_keys` (only overwrite
  `mcpServers.tanren`)

### 2.2 Codex Skills (`codex-skills`)

- **Path:** `{target}/<command>/SKILL.md` — **one directory per
  command**
- **Format:** YAML frontmatter (must include `name`, `description`)
  + markdown body
- **MCP config:** `.codex/config.toml` — TOML, per-server section:
  ```toml
  [mcp_servers.tanren]
  command = "tanren-mcp"
  args = ["serve"]
  env = { TANREN_CONFIG = "./tanren.yml" }
  startup_timeout_sec = 10
  tool_timeout_sec = 60
  enabled = true
  ```
- **Merge (commands):** `destructive`
- **Merge (config):** `preserve_other_keys` (only overwrite
  `[mcp_servers.tanren]` section)
- Codex command rendering targets `.codex/skills/*/SKILL.md`.
  `AGENTS.md` is a separate shared-instructions convention and is not
  touched by `tanren install`.

### 2.3 OpenCode (`opencode`)

- **Path:** `{target}/<command>.md`
- **Format:** YAML frontmatter with the prompt body **inside the
  `template` field** (not the markdown body):
  ```yaml
  ---
  description: "…"
  agent: "…"
  model: "…"
  subtask: false
  template: |
    <entire prompt body goes here>
  ---
  ```
  The markdown body below the frontmatter is empty or ignored.
- **MCP config:** `opencode.json` (JSON, top-level `mcp` object)
- **Merge (commands):** `destructive`
- **Merge (config):** `preserve_other_keys`

### 2.4 Standards baseline (`standards-baseline`)

- **Path:** `{target}/<category>/<standard>.md`
- **Format:** YAML frontmatter (`name`, `category`, `applies_to`,
  `applies_to_languages`, `applies_to_domains`, `importance`) +
  markdown body
- **Merge:** `preserve_existing` — only create missing files; never
  overwrite. Intentional upgrades use `tanren update standards`
  (future lane).

---

## 3. Renderer architecture

### 3.1 Canonical command IR

```rust
pub struct RenderedCommand {
    pub name: String,                  // command name, e.g. "do-task"
    pub role: CommandRole,             // conversation | implementation | audit | adherence | feedback | meta
    pub orchestration_loop: bool,
    pub autonomy: Autonomy,            // interactive | autonomous
    pub declared_variables: Vec<String>,
    pub declared_tools: Vec<String>,
    pub required_capabilities: Vec<String>,
    pub produces_evidence: Vec<String>,
    pub extensions: BTreeMap<String, serde_yaml::Value>,
    pub body: String,                  // fully substituted markdown
}
```

### 3.2 Format trait

```rust
pub trait InstallTargetFormat {
    fn render_command(&self, cmd: &RenderedCommand)
        -> Result<Vec<WriteOp>, RenderError>;

    fn render_mcp_config(&self, servers: &[McpServerDecl])
        -> Result<Option<WriteOp>, RenderError>;

    fn merge_policy(&self) -> MergePolicy;
}
```

Shipped drivers:
- `ClaudeCode`
- `CodexSkills`
- `OpenCode`
- `StandardsBaseline`
- `ClaudeMcpJson` (config-only)
- `CodexConfigToml` (config-only)
- `OpenCodeJson` (config-only)

### 3.3 Write operations

```rust
pub struct WriteOp {
    pub path: PathBuf,
    pub content: Vec<u8>,
    pub mode: Option<u32>,         // Unix permission bits if special
    pub merge: MergePolicy,
}

pub enum MergePolicy {
    Destructive,
    PreserveExisting,
    PreserveOtherKeys { key_path: String },  // for merging into JSON/TOML
}
```

The installer executes writes atomically per file (tempfile + rename)
and collects results for dry-run output.

---

## 4. Template variable resolution

### 4.1 Taxonomy

| Variable | Source |
|---|---|
| `{{TASK_VERIFICATION_HOOK}}` | `methodology.variables.task_verification_hook` → `verification_hooks.do-task` → `task_gate_cmd` → `verification_hooks.default` → `gate_cmd` |
| `{{SPEC_VERIFICATION_HOOK}}` | analogous |
| `{{AUDIT_TASK_HOOK}}`, `{{ADHERE_TASK_HOOK}}`, `{{RUN_DEMO_HOOK}}`, `{{ADHERE_SPEC_HOOK}}` | per-phase override → task/spec fallback |
| `{{ISSUE_PROVIDER}}` | `methodology.variables.issue_provider` (required) |
| `{{ISSUE_REF_NOUN}}`, `{{PR_NOUN}}` | derived from issue_provider |
| `{{SPEC_ROOT}}`, `{{PRODUCT_ROOT}}`, `{{STANDARDS_ROOT}}` | optional; defaults `tanren/specs`, `tanren/product`, `tanren/standards` |
| `{{PROJECT_LANGUAGE}}` | `methodology.variables.project_language` (required) |
| `{{AGENT_CLI_NOUN}}` | `the agent CLI` by default |
| `{{TASK_TOOL_BINDING}}` | install-target `binding` (`mcp` | `cli`) |
| `{{PHASE_EVENTS_FILE}}` | `{spec_folder}/phase-events.jsonl` |
| `{{READONLY_ARTIFACT_BANNER}}` | fixed prose |
| `{{PILLAR_LIST}}` | effective rubric pillar ids: `tanren/rubric.yml` (preferred) → `tanren.yml methodology.rubric` (canonical) → built-in pillar ids |
| `{{REQUIRED_GUARDS}}` | effective `methodology.task_complete_requires` after profile overrides (`tanren install --profile`) |

The installer must resolve both variables at install time from the
active config/rubric state; hardcoded literals are non-compliant.

### 4.2 Validation

- Unknown `{{VAR}}` in template → hard error with file:line.
- Declared-but-unused variable in template frontmatter → hard error.
- Used-but-not-declared variable → hard error (keeps templates
  self-describing).
- `--strict` additionally fails on any warning (e.g., unused config
  value).

---

## 5. Config

`tanren.yml`:

```yaml
methodology:
  task_complete_requires: [gate_checked, audited, adherent]

  source:
    path: commands                 # relative to repo root; overridable

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
    # spec_root, product_root, standards_root, agent_cli_noun: defaults
```

---

## 6. CLI surface

```
tanren install
    [--profile <name>]              # tanren.yml profile (optional)
    [--config <path>]               # override tanren.yml location
    [--source <path>]               # override commands source dir
    [--target <label>...]           # subset of configured targets
    [--dry-run]                     # show diffs; write nothing
    [--strict]                      # fail on warnings
```

**Exit codes:**
- `0` — success, all writes applied (or would be, under dry-run).
- `1` — config/render error (unknown variable, schema failure).
- `2` — target write error (filesystem, permission).
- `3` — dry-run detected pending changes (CI gate).
- `4` — validation error (standard missing required metadata, etc.).

**Logging:** `tracing` to stderr. Default level `INFO`; `--verbose`
bumps to `DEBUG`. **Never** writes to stdout except for machine-
readable output (future `--format json`).

---

## 7. Multi-target parity verification

Tests assert that, for a given source + config, the rendered
`body` (post-template-substitution) is semantically identical across
Claude Code, Codex Skills, and OpenCode outputs. Implementation:

1. Compute canonical form: strip format-specific wrappers; normalize
   whitespace; extract the `template` field for OpenCode.
2. Hash the canonical form.
3. Assert hashes match across targets for every command.

This lives as an integration test in
`crates/tanren-app-services/tests/install_parity.rs`.

---

## 8. Merge policies: details

### 8.1 Destructive

```
if target_file exists:
    overwrite
else:
    create
```

### 8.2 PreserveExisting

```
if target_file exists:
    skip (log at INFO)
else:
    create
```

### 8.3 PreserveOtherKeys { key_path }

Parse the existing file (JSON or TOML), update only the keys at
`key_path`, re-serialize. If parse fails and the file is empty/missing,
create from scratch. If parse fails on a non-empty file, fail with
`TargetConfigMalformed { path, reason }`.

---

## 9. Self-hosting

The tanren repo itself exercises all three agent targets:

```
.claude/commands/         ← committed, rendered from commands/
.codex/skills/            ← committed, rendered
.opencode/commands/       ← committed, rendered
.mcp.json                 ← committed, tanren-mcp registered
.codex/config.toml        ← committed
opencode.json             ← committed
```

`just install-commands` in the tanren repo regenerates all of the
above. `just install-commands-check` (run in `just ci`) asserts no
drift.

**Downstream repos do not inherit the `just` recipes.** They run
`tanren install` standalone and choose their own CI integration.

---

## 10. See also

- Tool surface whose MCP configuration is generated:
  [agent-tool-surface.md](agent-tool-surface.md)
- Orchestration flow (commands' runtime role):
  [orchestration-flow.md](orchestration-flow.md)
- Design rationale: [../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md](../rewrite/tasks/LANE-0.5-DESIGN-NOTES.md)
