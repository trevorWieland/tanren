//! `tanren.yml` methodology-section config model.
//!
//! Parsed from the repo-root `tanren.yml`. Drives the installer
//! (source / install_targets / mcp / variables) plus the runtime
//! `task_complete_requires` guard list.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use tanren_domain::methodology::pillar::ApplicableAt;
use tanren_domain::methodology::task::RequiredGuard;

/// Full methodology section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodologyConfig {
    #[serde(default = "default_required_guards")]
    pub task_complete_requires: Vec<RequiredGuard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceConfig>,
    #[serde(default)]
    pub standards: StandardsConfig,
    #[serde(default)]
    pub install_targets: Vec<InstallTarget>,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub rubric: MethodologyRubricConfig,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    /// Legacy config overlays. Kept only so callers can emit a clear
    /// validation error instead of silently ignoring old configs.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, MethodologyProfile>,
}

/// Standards install configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardsConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// A named override block that `tanren-cli install --profile NAME`
/// applies on top of the top-level `methodology` section. Every field
/// is optional — omitted fields leave the base value untouched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodologyProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_complete_requires: Option<Vec<RequiredGuard>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_targets: Option<Vec<InstallTarget>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rubric: Option<MethodologyRubricConfig>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, String>,
}

impl MethodologyProfile {
    /// Apply this profile's overrides to `base` in place. Variables
    /// merge (profile wins on collision); vector/scalar fields fully
    /// replace when set.
    pub fn apply(&self, base: &mut MethodologyConfig) {
        if let Some(v) = &self.task_complete_requires {
            base.task_complete_requires.clone_from(v);
        }
        if let Some(s) = &self.source {
            base.source = Some(s.clone());
        }
        if let Some(t) = &self.install_targets {
            base.install_targets.clone_from(t);
        }
        if let Some(m) = &self.mcp {
            base.mcp.clone_from(m);
        }
        if let Some(r) = &self.rubric {
            base.rubric.clone_from(r);
        }
        for (k, v) in &self.variables {
            base.variables.insert(k.clone(), v.clone());
        }
    }
}

/// Rubric configuration under `methodology.rubric`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodologyRubricConfig {
    #[serde(default)]
    pub pillars: Vec<MethodologyRubricPillar>,
    #[serde(default)]
    pub disable_builtin: Vec<String>,
}

impl MethodologyRubricConfig {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pillars.is_empty() && self.disable_builtin.is_empty()
    }
}

/// One rubric pillar override/addition row.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodologyRubricPillar {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_score: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passing_score: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applicable_at: Option<ApplicableAt>,
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
    TanrenConfig,
    ClaudeCode,
    CodexSkills,
    Opencode,
    #[serde(rename = "standards-baseline")]
    StandardsBaseline,
    StandardsProfile,
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
    #[serde(default)]
    pub security: McpSecurityConfig,
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

/// MCP capability-envelope security defaults emitted into install-time config.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpSecurityConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_issuer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_audience: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_public_key_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_private_key_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_max_ttl_secs: Option<u64>,
}

/// Top-level `tanren.yml` shape. Only the `methodology` section is
/// consumed here; other keys round-trip via `serde_yaml::Value`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TanrenConfig {
    pub methodology: MethodologyConfig,
    #[serde(default)]
    pub environment: EnvironmentConfig,
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_yaml::Value>,
}

/// Top-level `environment` block. Installer currently consumes only
/// the `default` profile's verification hook fields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    #[serde(default)]
    pub default: EnvironmentProfile,
}

/// Environment hook fields used to resolve verification command template variables.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentProfile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_gate_cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_gate_cmd: Option<String>,
    #[serde(default)]
    pub verification_hooks: BTreeMap<String, String>,
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
