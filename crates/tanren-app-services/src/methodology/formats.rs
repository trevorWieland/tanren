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
            InstallFormat::StandardsBaseline
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
            frontmatter_extra_string(cmd, "agent").unwrap_or_else(|| cmd.frontmatter.role.clone()),
        ),
    );
    fm.insert(
        "model".into(),
        serde_yaml::Value::String(
            frontmatter_extra_string(cmd, "model").unwrap_or_else(|| "default".into()),
        ),
    );
    fm.insert(
        "subtask".into(),
        serde_yaml::Value::Bool(frontmatter_extra_bool(cmd, "subtask").unwrap_or(false)),
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
    frontmatter_extra_string(cmd, "description")
        .unwrap_or_else(|| format!("Tanren methodology command `{}`", cmd.frontmatter.name))
}

fn frontmatter_extra_string(cmd: &RenderedCommand, key: &str) -> Option<String> {
    match cmd.frontmatter.extras.get(key) {
        Some(serde_yaml::Value::String(s)) if !s.trim().is_empty() => Some(s.clone()),
        _ => None,
    }
}

fn frontmatter_extra_bool(cmd: &RenderedCommand, key: &str) -> Option<bool> {
    match cmd.frontmatter.extras.get(key) {
        Some(serde_yaml::Value::Bool(v)) => Some(*v),
        Some(serde_yaml::Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
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
                "env": { "TANREN_CONFIG": "./tanren.yml" },
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
    env.insert(
        "TANREN_CONFIG".into(),
        toml::Value::String("./tanren.yml".into()),
    );
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
                "command": server_command,
                "args": server_args,
                "env": { "TANREN_CONFIG": "./tanren.yml" },
            }),
        );
    let mut out = serde_json::to_string_pretty(&root)
        .map_err(|e| MethodologyError::Internal(e.to_string()))?;
    out.push('\n');
    Ok(out.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::methodology::renderer::RenderedCommand;
    use crate::methodology::source::{CommandFamily, CommandFrontmatter};

    fn rendered(name: &str, body: &str) -> RenderedCommand {
        RenderedCommand {
            name: name.into(),
            family: CommandFamily::SpecLoop,
            frontmatter: CommandFrontmatter {
                name: name.into(),
                role: "implementation".into(),
                orchestration_loop: true,
                autonomy: "autonomous".into(),
                declared_variables: vec![],
                declared_tools: vec![],
                required_capabilities: vec![],
                produces_evidence: vec![],
                extras: Default::default(),
            },
            body: body.into(),
        }
    }

    #[test]
    fn claude_code_layout() {
        let r = rendered("do-task", "body\n");
        let art = claude_code(&r, Path::new(".claude/commands")).expect("ok");
        assert_eq!(art.dest, Path::new(".claude/commands/do-task.md"));
        let s = String::from_utf8(art.bytes).expect("utf8");
        assert!(s.starts_with("---\n"));
        assert!(s.ends_with("body\n"));
    }

    #[test]
    fn codex_skills_directory_per_command() {
        let r = rendered("do-task", "body\n");
        let art = codex_skills(&r, Path::new(".codex/skills")).expect("ok");
        assert_eq!(art.dest, Path::new(".codex/skills/do-task/SKILL.md"));
    }

    #[test]
    fn opencode_has_template_field() {
        let r = rendered("do-task", "body\n");
        let art = opencode(&r, Path::new(".opencode/commands")).expect("ok");
        let s = String::from_utf8(art.bytes).expect("utf8");
        assert!(s.contains("description"));
        assert!(s.contains("agent"));
        assert!(s.contains("model"));
        assert!(s.contains("subtask"));
        assert!(s.contains("template"));
        assert!(s.contains("body"));
    }

    #[test]
    fn mcp_json_preserves_other_keys() {
        let existing = "{\n  \"other\": 42\n}";
        let bytes = claude_mcp_json(Some(existing), "tanren-mcp", &[]).expect("ok");
        let s = String::from_utf8(bytes).expect("utf8");
        assert!(s.contains("\"other\": 42"));
        assert!(s.contains("\"tanren\""));
        assert!(s.contains("\"env\""));
    }

    #[test]
    fn codex_skill_includes_description() {
        let r = rendered("do-task", "body\n");
        let art = codex_skills(&r, Path::new(".codex/skills")).expect("ok");
        let s = String::from_utf8(art.bytes).expect("utf8");
        assert!(s.contains("description"));
    }

    #[test]
    fn codex_config_writes_timeout_and_enabled_fields() {
        let bytes = codex_config_toml(None, "tanren-mcp", &["serve".into()]).expect("ok");
        let s = String::from_utf8(bytes).expect("utf8");
        assert!(s.contains("startup_timeout_sec = 10"));
        assert!(s.contains("tool_timeout_sec = 60"));
        assert!(s.contains("enabled = true"));
        assert!(s.contains("TANREN_CONFIG"));
    }
}
