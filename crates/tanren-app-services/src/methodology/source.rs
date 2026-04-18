//! Command-source loader.
//!
//! Reads `commands/spec/*.md` and `commands/project/*.md`, parses each
//! file's YAML frontmatter into [`CommandSource`], and surfaces a
//! typed catalog for the installer + renderer.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use tanren_domain::methodology::evidence::frontmatter::{FrontmatterError, parse_typed};

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
    /// Permit forward-compatible frontmatter fields.
    #[serde(flatten)]
    pub extras: std::collections::BTreeMap<String, serde_yaml::Value>,
}

fn default_autonomy() -> String {
    "autonomous".into()
}

/// Load the full command catalog from a `commands/` root.
///
/// Recognizes `commands/spec/*.md` and `commands/project/*.md`. Files
/// outside those subdirs are ignored; a `README.md` at the root is
/// permitted but not loaded.
///
/// # Errors
/// Returns [`MethodologyError::Io`] on directory I/O failure and
/// [`MethodologyError::Validation`] on malformed frontmatter.
pub fn load_catalog(root: &Path) -> MethodologyResult<Vec<CommandSource>> {
    let mut out = Vec::new();
    for family in [CommandFamily::SpecLoop, CommandFamily::Project] {
        let dir = root.join(family.subdir());
        if !dir.is_dir() {
            continue;
        }
        let entries = std::fs::read_dir(&dir).map_err(|source| MethodologyError::Io {
            path: dir.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| MethodologyError::Io {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            out.push(load_one(&path, family)?);
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn load_one(path: &Path, family: CommandFamily) -> MethodologyResult<CommandSource> {
    let text = std::fs::read_to_string(path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let (fm, body): (CommandFrontmatter, String) = parse_typed(&text).map_err(|e| match e {
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
