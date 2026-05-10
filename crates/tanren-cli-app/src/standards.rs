//! CLI standards inspection command.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use tanren_configuration_secrets::{MethodologyProfile, ProjectMethodologyConfig};
use thiserror::Error;

use crate::install::resolve_repo_relative_path;

const PROJECT_METHODOLOGY_CONFIG_REPO_PATH: &str = ".tanren/project-methodology.toml";

/// `tanren-cli standards` command arguments.
#[derive(Debug, Clone, Args)]
pub(crate) struct StandardsCommand {
    #[command(subcommand)]
    action: StandardsAction,
}

impl StandardsCommand {
    /// Dispatch standards subcommands.
    pub(crate) fn run(&self) -> Result<(), StandardsCommandError> {
        match &self.action {
            StandardsAction::Inspect(command) => command.run(),
        }
    }
}

/// Supported `tanren-cli standards` subcommands.
#[derive(Debug, Clone, Subcommand)]
enum StandardsAction {
    /// Inspect standards from the repository's configured standards root.
    Inspect(StandardsInspectCommand),
}

/// `tanren-cli standards inspect` command arguments.
#[derive(Debug, Clone, Args)]
struct StandardsInspectCommand {
    /// Repository path to inspect.
    #[arg(long)]
    repo: PathBuf,
}

impl StandardsInspectCommand {
    fn run(&self) -> Result<(), StandardsCommandError> {
        let report = inspect_standards(&self.repo)?;

        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        writeln!(
            handle,
            "status=ok command=standards.inspect repo={} profile={} standards_root={} standards_count={} first_standard_name={} first_standard_path={}",
            display_repository_argument(&self.repo),
            report.profile,
            report.standards_root,
            report.standards_count,
            report.first_standard_name,
            report.first_standard_path,
        )
        .map_err(|source| StandardsCommandError::StdoutWriteFailure { source })?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct StandardsInspectReport {
    profile: String,
    standards_root: String,
    standards_count: usize,
    first_standard_name: String,
    first_standard_path: String,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub(crate) enum StandardsCommandError {
    /// Input validation failed before inspecting standards content.
    #[error("error: validation_failed - {source}")]
    ValidationFailed {
        #[source]
        source: StandardsError,
    },
    /// Configured standards directory is missing.
    #[error("error: standards_missing - {source}")]
    StandardsMissing {
        #[source]
        source: StandardsError,
    },
    /// Standards content or frontmatter parsing failed.
    #[error("error: standards_parse_failed - {source}")]
    StandardsParseFailed {
        #[source]
        source: StandardsError,
    },
    /// Emitting success output to stdout failed.
    #[error("error: output_write_failed - write standards report to stdout: {source}")]
    StdoutWriteFailure {
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Error)]
pub(crate) enum StandardsError {
    #[error("repository path is invalid or inaccessible: '{path}'")]
    InvalidRepositoryPath { path: String },
    #[error("repository path does not exist or is not a directory: '{path}'")]
    RepositoryPathNotDirectory { path: String },
    #[error("failed reading '{path}': {message}")]
    ReadFailure { path: String, message: String },
    #[error("failed to parse project methodology config '{path}' as TOML: {message}")]
    ProjectMethodologyConfigParse { path: String, message: String },
    #[error("configured standards root '{path}' is invalid: {message}")]
    InvalidConfiguredStandardsRoot { path: String, message: String },
    #[error("path is not repository-relative: '{path}'")]
    NonRepositoryRelativePath { path: String },
    #[error("configured standards root is missing: '{path}'")]
    StandardsRootMissing { path: String },
    #[error("no standards markdown files found under configured standards root '{path}'")]
    NoStandardsFiles { path: String },
    #[error("failed to parse standards frontmatter in '{path}': {message}")]
    FrontmatterParse { path: String, message: String },
}

#[derive(Debug, Clone)]
struct ParsedStandard {
    path: String,
    name: String,
}

fn inspect_standards(repository: &Path) -> Result<StandardsInspectReport, StandardsCommandError> {
    let repository_argument = display_repository_argument(repository);
    let repository_root =
        repository
            .canonicalize()
            .map_err(|_| StandardsCommandError::ValidationFailed {
                source: StandardsError::InvalidRepositoryPath {
                    path: repository_argument.clone(),
                },
            })?;

    if !repository_root.is_dir() {
        return Err(StandardsCommandError::ValidationFailed {
            source: StandardsError::RepositoryPathNotDirectory {
                path: repository_argument,
            },
        });
    }

    let config_path = repository_root.join(PROJECT_METHODOLOGY_CONFIG_REPO_PATH);
    let config_text = fs::read_to_string(&config_path).map_err(|source| {
        StandardsCommandError::ValidationFailed {
            source: StandardsError::ReadFailure {
                path: PROJECT_METHODOLOGY_CONFIG_REPO_PATH.to_owned(),
                message: source.to_string(),
            },
        }
    })?;

    let config = ProjectMethodologyConfig::from_toml(&config_text).map_err(|source| {
        StandardsCommandError::ValidationFailed {
            source: StandardsError::ProjectMethodologyConfigParse {
                path: PROJECT_METHODOLOGY_CONFIG_REPO_PATH.to_owned(),
                message: source.to_string(),
            },
        }
    })?;

    let standards_root_relative = config.standards_root.as_str().to_owned();
    let standards_root =
        resolve_repo_relative_path(&repository_root, config.standards_root.as_str()).map_err(
            |source| StandardsCommandError::ValidationFailed {
                source: StandardsError::InvalidConfiguredStandardsRoot {
                    path: standards_root_relative.clone(),
                    message: source.to_string(),
                },
            },
        )?;

    if !standards_root.is_dir() {
        return Err(StandardsCommandError::StandardsMissing {
            source: StandardsError::StandardsRootMissing {
                path: standards_root_relative,
            },
        });
    }

    let mut parsed = Vec::new();
    collect_standards(&repository_root, &standards_root, &mut parsed)?;
    parsed.sort_by(|left, right| left.path.cmp(&right.path));

    let Some(first) = parsed.first() else {
        return Err(StandardsCommandError::StandardsMissing {
            source: StandardsError::NoStandardsFiles {
                path: config.standards_root.as_str().to_owned(),
            },
        });
    };

    Ok(StandardsInspectReport {
        profile: methodology_profile_name(config.profile).to_owned(),
        standards_root: config.standards_root.as_str().to_owned(),
        standards_count: parsed.len(),
        first_standard_name: first.name.clone(),
        first_standard_path: first.path.clone(),
    })
}

fn collect_standards(
    repository_root: &Path,
    directory: &Path,
    parsed: &mut Vec<ParsedStandard>,
) -> Result<(), StandardsCommandError> {
    let directory_path =
        to_repo_relative_path(repository_root, directory).map_err(validation_failed)?;
    let entries =
        fs::read_dir(directory).map_err(|source| StandardsCommandError::StandardsMissing {
            source: StandardsError::ReadFailure {
                path: directory_path.clone(),
                message: source.to_string(),
            },
        })?;

    for entry_result in entries {
        let entry = entry_result.map_err(|source| StandardsCommandError::StandardsMissing {
            source: StandardsError::ReadFailure {
                path: directory_path.clone(),
                message: source.to_string(),
            },
        })?;
        let path = entry.path();
        let path_relative =
            to_repo_relative_path(repository_root, &path).map_err(validation_failed)?;
        let file_type =
            entry
                .file_type()
                .map_err(|source| StandardsCommandError::StandardsMissing {
                    source: StandardsError::ReadFailure {
                        path: path_relative.clone(),
                        message: source.to_string(),
                    },
                })?;

        if file_type.is_dir() {
            collect_standards(repository_root, &path, parsed)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }

        let standard_path_relative = path_relative;
        let raw = fs::read_to_string(&path).map_err(|source| {
            StandardsCommandError::StandardsParseFailed {
                source: StandardsError::ReadFailure {
                    path: standard_path_relative.clone(),
                    message: source.to_string(),
                },
            }
        })?;
        let standard_name = parse_standard_name(&raw).map_err(|source| {
            StandardsCommandError::StandardsParseFailed {
                source: StandardsError::FrontmatterParse {
                    path: standard_path_relative.clone(),
                    message: source,
                },
            }
        })?;

        parsed.push(ParsedStandard {
            path: standard_path_relative,
            name: standard_name,
        });
    }

    Ok(())
}

fn parse_standard_name(content: &str) -> Result<String, String> {
    let frontmatter = extract_frontmatter(content)?;
    let name = parse_frontmatter_name(frontmatter)?;
    let name = name.trim();
    if name.is_empty() {
        return Err("frontmatter 'name' must not be empty".to_owned());
    }
    Ok(name.to_owned())
}

fn extract_frontmatter(content: &str) -> Result<&str, String> {
    let mut lines = content.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Err("missing opening frontmatter delimiter".to_owned());
    };
    if trim_line_ending(first) != "---" {
        return Err("missing opening frontmatter delimiter".to_owned());
    }

    let body_start = first.len();
    let mut body_end = body_start;

    for line in lines {
        if trim_line_ending(line) == "---" {
            return content
                .get(body_start..body_end)
                .ok_or_else(|| "invalid frontmatter byte bounds".to_owned());
        }
        body_end += line.len();
    }

    Err("missing closing frontmatter delimiter".to_owned())
}

fn parse_frontmatter_name(frontmatter: &str) -> Result<&str, String> {
    let mut name: Option<&str> = None;
    let mut in_list = false;

    for raw_line in frontmatter.lines() {
        let line = trim_line_ending(raw_line);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if in_list && trimmed.starts_with("- ") {
            continue;
        }
        in_list = false;

        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(format!(
                "invalid frontmatter line: expected top-level key, got '{trimmed}'"
            ));
        }

        let Some((key_raw, value_raw)) = trimmed.split_once(':') else {
            return Err(format!(
                "invalid frontmatter line: expected key:value, got '{trimmed}'"
            ));
        };

        let key = key_raw.trim();
        if key.is_empty() {
            return Err("invalid frontmatter line: empty key before ':'".to_owned());
        }

        let value = value_raw.trim();
        if value.is_empty() {
            in_list = true;
            continue;
        }

        if key == "name" {
            if name.is_some() {
                return Err("frontmatter contains duplicate 'name' entries".to_owned());
            }
            name = Some(unquote_yaml_scalar(value));
        }
    }

    name.ok_or_else(|| "frontmatter missing required 'name' key".to_owned())
}

fn unquote_yaml_scalar(value: &str) -> &str {
    let quoted = (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''));
    if quoted && value.len() >= 2 {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn trim_line_ending(line: &str) -> &str {
    line.trim_end_matches('\n').trim_end_matches('\r')
}

const fn methodology_profile_name(profile: MethodologyProfile) -> &'static str {
    match profile {
        MethodologyProfile::RustCargo => "rust-cargo",
    }
}

fn to_repo_relative_path(repository_root: &Path, path: &Path) -> Result<String, StandardsError> {
    let relative = path.strip_prefix(repository_root).map_err(|_| {
        StandardsError::NonRepositoryRelativePath {
            path: display_repository_argument(path),
        }
    })?;
    Ok(relative.display().to_string())
}

fn validation_failed(source: StandardsError) -> StandardsCommandError {
    StandardsCommandError::ValidationFailed { source }
}

fn display_repository_argument(path: &Path) -> String {
    if path.is_absolute() {
        "<redacted-absolute-path>".to_owned()
    } else {
        path.display().to_string()
    }
}
