//! Embedded command-source loader.
//!
//! Parses compiled-in `commands/spec/*.md` and `commands/project/*.md`
//! assets into a typed catalog for the installer + renderer.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use tanren_domain::methodology::capability::ToolCapability;
use tanren_domain::methodology::descriptor_by_name;
use tanren_domain::methodology::evidence::frontmatter::{FrontmatterError, parse_typed};

use super::assets::EmbeddedAsset;
use super::errors::{MethodologyError, MethodologyResult};

/// One source command (before rendering).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSource {
    pub name: String,
    pub family: CommandFamily,
    pub frontmatter: CommandFrontmatter,
    pub body: String,
    pub source_path: PathBuf,
}

/// Which command tree a command was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandFamily {
    SpecLoop,
    Project,
}

impl CommandFamily {
    /// Short relative subdir name (`spec` or `project`).
    #[must_use]
    pub const fn subdir(self) -> &'static str {
        match self {
            Self::SpecLoop => "spec",
            Self::Project => "project",
        }
    }
}

/// YAML frontmatter of a source command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandFrontmatter {
    pub name: String,
    pub role: String,
    #[serde(default)]
    pub orchestration_loop: bool,
    #[serde(default = "default_autonomy")]
    pub autonomy: String,
    #[serde(default)]
    pub declared_variables: Vec<String>,
    #[serde(default)]
    pub declared_tools: Vec<String>,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
    #[serde(default)]
    pub produces_evidence: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub subtask: Option<bool>,
    /// Explicit extension namespace for forward-compatible metadata.
    #[serde(default)]
    pub extensions: std::collections::BTreeMap<String, serde_yaml::Value>,
}

fn default_autonomy() -> String {
    "autonomous".into()
}

/// Load the full command catalog from embedded distribution assets.
///
/// Recognizes `commands/spec/*.md` and `commands/project/*.md`.
///
/// # Errors
/// Returns [`MethodologyError::Validation`] on malformed frontmatter
/// or command contract violations.
pub fn load_embedded_catalog() -> MethodologyResult<Vec<CommandSource>> {
    load_catalog_from_assets(super::assets::COMMAND_ASSETS)
}

/// Load command assets from an explicit in-memory asset set. This is
/// used by tests to assert the embedded manifest shape without
/// touching the filesystem.
///
/// # Errors
/// Returns [`MethodologyError::Validation`] on malformed frontmatter
/// or command contract violations.
pub fn load_catalog_from_assets(assets: &[EmbeddedAsset]) -> MethodologyResult<Vec<CommandSource>> {
    let mut out = Vec::new();
    for asset in assets {
        let path = Path::new(asset.path);
        let Some(family) = command_family_for_asset(path) else {
            continue;
        };
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        out.push(load_one_from_text(path, family, asset.contents)?);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    for command in &out {
        validate_command_contract(command)?;
    }
    Ok(out)
}

fn load_one_from_text(
    path: &Path,
    family: CommandFamily,
    text: &str,
) -> MethodologyResult<CommandSource> {
    let (fm, body): (CommandFrontmatter, String) = parse_typed(text).map_err(|e| match e {
        FrontmatterError::MissingOpener | FrontmatterError::MissingCloser => {
            MethodologyError::Validation(format!(
                "malformed frontmatter in {}: {e}",
                path.display()
            ))
        }
        FrontmatterError::InvalidYaml { source } => MethodologyError::Validation(format!(
            "invalid frontmatter YAML in {}: {source}",
            path.display()
        )),
        FrontmatterError::SchemaError { reason } => MethodologyError::Validation(format!(
            "frontmatter schema error in {}: {reason}",
            path.display()
        )),
    })?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(&fm.name)
        .to_owned();
    Ok(CommandSource {
        name,
        family,
        frontmatter: fm,
        body,
        source_path: path.to_path_buf(),
    })
}

fn command_family_for_asset(path: &Path) -> Option<CommandFamily> {
    let mut components = path.components();
    match (
        components.next().and_then(|c| c.as_os_str().to_str()),
        components.next().and_then(|c| c.as_os_str().to_str()),
    ) {
        (Some("commands"), Some("spec")) => Some(CommandFamily::SpecLoop),
        (Some("commands"), Some("project")) => Some(CommandFamily::Project),
        _ => None,
    }
}

fn validate_command_contract(command: &CommandSource) -> MethodologyResult<()> {
    let mut required = std::collections::BTreeSet::new();
    for capability in &command.frontmatter.required_capabilities {
        let Some(parsed) = ToolCapability::from_tag(capability) else {
            return Err(MethodologyError::Validation(format!(
                "command `{}` ({}): unknown required capability `{capability}`",
                command.name,
                command.source_path.display()
            )));
        };
        required.insert(parsed);
    }
    for tool in &command.frontmatter.declared_tools {
        let Some(implied) = implied_capability_for_tool(tool) else {
            return Err(MethodologyError::Validation(format!(
                "command `{}` ({}): unknown declared tool `{tool}`",
                command.name,
                command.source_path.display()
            )));
        };
        if !required.contains(&implied) {
            return Err(MethodologyError::Validation(format!(
                "command `{}` ({}): declared tool `{tool}` requires capability `{}` but it is missing from required_capabilities",
                command.name,
                command.source_path.display(),
                implied.tag(),
            )));
        }
    }
    Ok(())
}

fn implied_capability_for_tool(tool: &str) -> Option<ToolCapability> {
    descriptor_by_name(tool).map(|descriptor| descriptor.capability)
}
