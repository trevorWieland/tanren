//! Command-source loader.
//!
//! Reads `commands/spec/*.md` and `commands/project/*.md`, parses each
//! file's YAML frontmatter into [`CommandSource`], and surfaces a
//! typed catalog for the installer + renderer.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use tanren_domain::methodology::capability::ToolCapability;
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
    for command in &out {
        validate_command_contract(command)?;
    }
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
    Some(match tool {
        "create_task" => ToolCapability::TaskCreate,
        "start_task" => ToolCapability::TaskStart,
        "complete_task" | "mark_task_guard_satisfied" => ToolCapability::TaskComplete,
        "revise_task" => ToolCapability::TaskRevise,
        "abandon_task" => ToolCapability::TaskAbandon,
        "list_tasks" => ToolCapability::TaskRead,
        "add_finding" => ToolCapability::FindingAdd,
        "record_rubric_score" => ToolCapability::RubricRecord,
        "record_non_negotiable_compliance" => ToolCapability::ComplianceRecord,
        "set_spec_title"
        | "set_spec_non_negotiables"
        | "add_spec_acceptance_criterion"
        | "set_spec_demo_environment"
        | "set_spec_dependencies"
        | "set_spec_base_branch"
        | "set_spec_relevance_context" => ToolCapability::SpecFrontmatter,
        "add_demo_step" | "mark_demo_step_skip" => ToolCapability::DemoFrontmatter,
        "append_demo_result" => ToolCapability::DemoResults,
        "add_signpost" => ToolCapability::SignpostAdd,
        "update_signpost_status" => ToolCapability::SignpostUpdate,
        "report_phase_outcome" => ToolCapability::PhaseOutcome,
        "escalate_to_blocker" => ToolCapability::PhaseEscalate,
        "post_reply_directive" => ToolCapability::FeedbackReply,
        "create_issue" => ToolCapability::IssueCreate,
        "list_relevant_standards" => ToolCapability::StandardRead,
        "record_adherence_finding" => ToolCapability::AdherenceRecord,
        _ => return None,
    })
}
