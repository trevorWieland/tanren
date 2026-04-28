//! Per-target format drivers.
//!
//! Each driver consumes a [`RenderedCommand`] and emits a
//! target-specific bytes + destination-path pair. The installer
//! handles I/O (atomic tempfile+rename); drivers are pure.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::config::InstallFormat;
use super::errors::{MethodologyError, MethodologyResult};
use super::renderer::RenderedCommand;

/// One planned write per rendered command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedArtifact {
    pub dest: PathBuf,
    pub bytes: Vec<u8>,
}

/// Render `commands` against the given install format + destination
/// root. Returns one [`RenderedArtifact`] per command.
///
/// # Errors
/// Returns [`MethodologyError::Internal`] if YAML / JSON / TOML
/// serialization fails for a target format.
pub fn render_commands(
    commands: &[RenderedCommand],
    format: InstallFormat,
    dest_root: &Path,
) -> MethodologyResult<Vec<RenderedArtifact>> {
    let mut out = Vec::with_capacity(commands.len());
    for cmd in commands {
        out.push(match format {
            InstallFormat::ClaudeCode => claude_code(cmd, dest_root)?,
            InstallFormat::CodexSkills => codex_skills(cmd, dest_root)?,
            InstallFormat::Opencode => opencode(cmd, dest_root)?,
            InstallFormat::TanrenConfig
            | InstallFormat::StandardsBaseline
            | InstallFormat::StandardsProfile
            | InstallFormat::ClaudeMcpJson
            | InstallFormat::CodexConfigToml
            | InstallFormat::OpencodeJson => {
                return Err(MethodologyError::Validation(format!(
                    "format {format:?} does not accept commands; \
                     use the config-writer entry points"
                )));
            }
        });
    }
    Ok(out)
}

/// Claude Code: `<root>/<name>.md` with YAML frontmatter + md body.
fn claude_code(cmd: &RenderedCommand, root: &Path) -> MethodologyResult<RenderedArtifact> {
    let fm = claude_frontmatter(cmd)?;
    let yaml = serde_yaml::to_string(&fm).map_err(|e| MethodologyError::Internal(e.to_string()))?;
    let mut bytes = Vec::with_capacity(yaml.len() + cmd.body.len() + 16);
    bytes.extend(b"---\n");
    bytes.extend(yaml.as_bytes());
    if !yaml.ends_with('\n') {
        bytes.extend(b"\n");
    }
    bytes.extend(b"---\n");
    bytes.extend(cmd.body.as_bytes());
    if !cmd.body.ends_with('\n') {
        bytes.extend(b"\n");
    }
    Ok(RenderedArtifact {
        dest: root.join(format!("{}.md", cmd.name)),
        bytes,
    })
}

/// Codex Skills: `<root>/<name>/SKILL.md` (directory-per-command).
fn codex_skills(cmd: &RenderedCommand, root: &Path) -> MethodologyResult<RenderedArtifact> {
    let fm = codex_frontmatter(cmd)?;
    let yaml = serde_yaml::to_string(&fm).map_err(|e| MethodologyError::Internal(e.to_string()))?;
    let mut bytes = Vec::with_capacity(yaml.len() + cmd.body.len() + 16);
    bytes.extend(b"---\n");
    bytes.extend(yaml.as_bytes());
    if !yaml.ends_with('\n') {
        bytes.extend(b"\n");
    }
    bytes.extend(b"---\n");
    bytes.extend(cmd.body.as_bytes());
    if !cmd.body.ends_with('\n') {
        bytes.extend(b"\n");
    }
    Ok(RenderedArtifact {
        dest: root.join(&cmd.name).join("SKILL.md"),
        bytes,
    })
}

/// OpenCode: `<root>/<name>.md` with the prompt body in a `template`
/// frontmatter field.
fn opencode(cmd: &RenderedCommand, root: &Path) -> MethodologyResult<RenderedArtifact> {
    let mut fm: BTreeMap<String, serde_yaml::Value> = BTreeMap::new();
    fm.insert(
        "description".into(),
        serde_yaml::Value::String(command_description(cmd)),
    );
    fm.insert(
        "agent".into(),
        serde_yaml::Value::String(
            cmd.frontmatter
                .agent
                .clone()
                .unwrap_or_else(|| cmd.frontmatter.role.clone()),
        ),
    );
    fm.insert(
        "model".into(),
        serde_yaml::Value::String(
            cmd.frontmatter
                .model
                .clone()
                .unwrap_or_else(|| "default".into()),
        ),
    );
    fm.insert(
        "subtask".into(),
        serde_yaml::Value::Bool(cmd.frontmatter.subtask.unwrap_or(false)),
    );
    fm.insert(
        "template".into(),
        serde_yaml::Value::String(cmd.body.clone()),
    );
    let yaml = serde_yaml::to_string(&fm).map_err(|e| MethodologyError::Internal(e.to_string()))?;
    let mut bytes = Vec::new();
    bytes.extend(b"---\n");
    bytes.extend(yaml.as_bytes());
    if !yaml.ends_with('\n') {
        bytes.extend(b"\n");
    }
    bytes.extend(b"---\n");
    Ok(RenderedArtifact {
        dest: root.join(format!("{}.md", cmd.name)),
        bytes,
    })
}

#[derive(Serialize)]
struct ClaudeFrontmatter<'a> {
    name: &'a str,
    role: &'a str,
    orchestration_loop: bool,
    autonomy: &'a str,
    declared_variables: &'a [String],
    declared_tools: &'a [String],
    required_capabilities: &'a [String],
    produces_evidence: &'a [String],
}

#[derive(Serialize)]
struct CodexFrontmatter<'a> {
    name: &'a str,
    description: String,
    role: &'a str,
    orchestration_loop: bool,
    autonomy: &'a str,
    declared_variables: &'a [String],
    declared_tools: &'a [String],
    required_capabilities: &'a [String],
    produces_evidence: &'a [String],
}

fn claude_frontmatter<'a>(cmd: &'a RenderedCommand) -> MethodologyResult<ClaudeFrontmatter<'a>> {
    Ok(ClaudeFrontmatter {
        name: &cmd.frontmatter.name,
        role: &cmd.frontmatter.role,
        orchestration_loop: cmd.frontmatter.orchestration_loop,
        autonomy: &cmd.frontmatter.autonomy,
        declared_variables: &cmd.frontmatter.declared_variables,
        declared_tools: &cmd.frontmatter.declared_tools,
        required_capabilities: &cmd.frontmatter.required_capabilities,
        produces_evidence: &cmd.frontmatter.produces_evidence,
    })
}

fn codex_frontmatter<'a>(cmd: &'a RenderedCommand) -> MethodologyResult<CodexFrontmatter<'a>> {
    Ok(CodexFrontmatter {
        name: &cmd.frontmatter.name,
        description: command_description(cmd),
        role: &cmd.frontmatter.role,
        orchestration_loop: cmd.frontmatter.orchestration_loop,
        autonomy: &cmd.frontmatter.autonomy,
        declared_variables: &cmd.frontmatter.declared_variables,
        declared_tools: &cmd.frontmatter.declared_tools,
        required_capabilities: &cmd.frontmatter.required_capabilities,
        produces_evidence: &cmd.frontmatter.produces_evidence,
    })
}

fn command_description(cmd: &RenderedCommand) -> String {
    cmd.frontmatter
        .description
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .map_or_else(
            || format!("Tanren methodology command `{}`", cmd.frontmatter.name),
            str::to_owned,
        )
}

/// `.mcp.json` writer — merges a `tanren` server stanza into existing
/// JSON while leaving other keys untouched.
///
/// # Errors
/// Returns [`MethodologyError::Internal`] on JSON serialization
/// failure.
pub fn claude_mcp_json(
    existing: Option<&str>,
    server_command: &str,
    server_args: &[String],
    server_env: &BTreeMap<String, String>,
) -> MethodologyResult<Vec<u8>> {
    let mut root: serde_json::Value = match existing {
        Some(s) if !s.trim().is_empty() => serde_json::from_str(s)
            .map_err(|e| MethodologyError::Validation(format!("existing .mcp.json: {e}")))?,
        _ => serde_json::json!({}),
    };
    let servers = root
        .as_object_mut()
        .ok_or_else(|| MethodologyError::Validation(".mcp.json root must be an object".into()))?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    servers
        .as_object_mut()
        .ok_or_else(|| MethodologyError::Validation("mcpServers must be an object".into()))?
        .insert(
            "tanren".into(),
            serde_json::json!({
                "command": server_command,
                "args": server_args,
                "env": server_env,
            }),
        );
    let mut out = serde_json::to_string_pretty(&root)
        .map_err(|e| MethodologyError::Internal(e.to_string()))?;
    out.push('\n');
    Ok(out.into_bytes())
}

/// `.codex/config.toml` writer — merges `[mcp_servers.tanren]` into
/// existing TOML.
///
/// # Errors
/// Returns a typed error on malformed existing TOML.
pub fn codex_config_toml(
    existing: Option<&str>,
    server_command: &str,
    server_args: &[String],
    server_env: &BTreeMap<String, String>,
) -> MethodologyResult<Vec<u8>> {
    let mut doc: toml::Table = match existing {
        Some(s) if !s.trim().is_empty() => toml::from_str(s)
            .map_err(|e| MethodologyError::Validation(format!("existing config.toml: {e}")))?,
        _ => toml::Table::new(),
    };
    let servers = doc
        .entry("mcp_servers".to_string())
        .or_insert(toml::Value::Table(toml::Table::new()));
    let servers_tbl = servers
        .as_table_mut()
        .ok_or_else(|| MethodologyError::Validation("mcp_servers must be a table".into()))?;
    let mut t = toml::Table::new();
    t.insert("command".into(), toml::Value::String(server_command.into()));
    t.insert(
        "args".into(),
        toml::Value::Array(
            server_args
                .iter()
                .cloned()
                .map(toml::Value::String)
                .collect(),
        ),
    );
    let mut env = toml::Table::new();
    for (k, v) in server_env {
        env.insert(k.clone(), toml::Value::String(v.clone()));
    }
    t.insert("env".into(), toml::Value::Table(env));
    t.insert("startup_timeout_sec".into(), toml::Value::Integer(10));
    t.insert("tool_timeout_sec".into(), toml::Value::Integer(60));
    t.insert("enabled".into(), toml::Value::Boolean(true));
    servers_tbl.insert("tanren".into(), toml::Value::Table(t));
    let mut out =
        toml::to_string_pretty(&doc).map_err(|e| MethodologyError::Internal(e.to_string()))?;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out.into_bytes())
}

/// `opencode.json` writer — merges `mcp.tanren` into existing JSON.
///
/// # Errors
/// Returns [`MethodologyError::Validation`] on malformed existing JSON.
pub fn opencode_json(
    existing: Option<&str>,
    server_command: &str,
    server_args: &[String],
    server_env: &BTreeMap<String, String>,
) -> MethodologyResult<Vec<u8>> {
    let mut root: serde_json::Value = match existing {
        Some(s) if !s.trim().is_empty() => serde_json::from_str(s)
            .map_err(|e| MethodologyError::Validation(format!("existing opencode.json: {e}")))?,
        _ => serde_json::json!({}),
    };
    let mcp = root
        .as_object_mut()
        .ok_or_else(|| MethodologyError::Validation("opencode.json root must be object".into()))?
        .entry("mcp")
        .or_insert_with(|| serde_json::json!({}));
    mcp.as_object_mut()
        .ok_or_else(|| MethodologyError::Validation("mcp must be object".into()))?
        .insert(
            "tanren".into(),
            serde_json::json!({
                "type": "local",
                "command": std::iter::once(server_command.to_owned())
                    .chain(server_args.iter().cloned())
                    .collect::<Vec<_>>(),
                "enabled": true,
                "environment": server_env,
            }),
        );
    let mut out = serde_json::to_string_pretty(&root)
        .map_err(|e| MethodologyError::Internal(e.to_string()))?;
    out.push('\n');
    Ok(out.into_bytes())
}
