//! `tanren.yml` methodology-section config model.
//!
//! Parsed from the repo-root `tanren.yml`. Drives the installer
//! (source / install_targets / mcp / variables) plus the runtime
//! `task_complete_requires` guard list.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use tanren_domain::methodology::task::RequiredGuard;

/// Full methodology section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodologyConfig {
    #[serde(default = "default_required_guards")]
    pub task_complete_requires: Vec<RequiredGuard>,
    pub source: SourceConfig,
    #[serde(default)]
    pub install_targets: Vec<InstallTarget>,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
}

fn default_required_guards() -> Vec<RequiredGuard> {
    vec![
        RequiredGuard::GateChecked,
        RequiredGuard::Audited,
        RequiredGuard::Adherent,
    ]
}

/// Where the rendered commands come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceConfig {
    pub path: PathBuf,
}

/// One render target per the installer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallTarget {
    pub path: PathBuf,
    pub format: InstallFormat,
    #[serde(default)]
    pub binding: InstallBinding,
    #[serde(default)]
    pub merge_policy: MergePolicy,
}

/// Authorial format for a render target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallFormat {
    ClaudeCode,
    CodexSkills,
    Opencode,
    StandardsBaseline,
    ClaudeMcpJson,
    CodexConfigToml,
    OpencodeJson,
}

/// Tool-call binding the agent interface will reference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallBinding {
    #[default]
    Mcp,
    Cli,
    None,
}

/// Merge policy for a render target.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergePolicy {
    /// Overwrite on reinstall.
    #[default]
    Destructive,
    /// Only create missing; never overwrite existing.
    PreserveExisting,
    /// Merge tanren-owned sub-keys into existing JSON/TOML; leave
    /// other keys untouched.
    PreserveOtherKeys,
}

/// MCP registration configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mcp_transport")]
    pub transport: McpTransport,
    #[serde(default)]
    pub also_write_configs: Vec<InstallTarget>,
}

fn default_mcp_transport() -> McpTransport {
    McpTransport::Stdio
}

/// MCP transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpTransport {
    #[default]
    Stdio,
}

/// Top-level `tanren.yml` shape. Only the `methodology` section is
/// consumed here; other keys round-trip via `serde_yaml::Value`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TanrenConfig {
    pub methodology: MethodologyConfig,
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_yaml::Value>,
}

impl TanrenConfig {
    /// Parse a `tanren.yml` document.
    ///
    /// # Errors
    /// Returns a `serde_yaml::Error` on invalid input.
    pub fn from_yaml(input: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let yaml = r#"
methodology:
  source:
    path: commands
"#;
        let cfg = TanrenConfig::from_yaml(yaml).expect("parse");
        assert_eq!(cfg.methodology.source.path, PathBuf::from("commands"));
        assert_eq!(cfg.methodology.task_complete_requires.len(), 3);
    }
}
