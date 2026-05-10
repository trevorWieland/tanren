//! CLI standards inspection command.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Serialize;
use tanren_configuration_secrets::{
    ConfigSecretsError, EffectiveConfigurationMetadata, EffectiveConfigurationSettingFamily,
    ProjectMethodologyConfig, StandardsRoot,
};
use thiserror::Error;

use crate::install::{InstallError, resolve_repo_relative_path};
use scanner::scan_standards;

const PROJECT_METHODOLOGY_CONFIG_REPO_PATH: &str = ".tanren/project-methodology.toml";

mod scanner;

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
        let output = StandardsInspectSuccessReport {
            status: StandardsInspectReportStatus::Ok,
            command: StandardsInspectReportCommand::StandardsInspect,
            repository: display_repository_argument(&self.repo),
            profile: report.profile,
            standards_root: report.standards_root,
            standards_count: report.standards_count,
            first_standard_name: report.first_standard_name,
            first_standard_path: report.first_standard_path,
            effective_configuration: report.effective_configuration,
        };

        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        serde_json::to_writer(&mut handle, &output)
            .map_err(|source| StandardsCommandError::ReportSerializeFailure { source })?;
        writeln!(handle).map_err(|source| StandardsCommandError::StdoutWriteFailure { source })?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct StandardsInspectSuccessReport {
    status: StandardsInspectReportStatus,
    command: StandardsInspectReportCommand,
    repository: String,
    profile: String,
    standards_root: StandardsRoot,
    standards_count: usize,
    first_standard_name: String,
    first_standard_path: String,
    effective_configuration: StandardsInspectEffectiveConfigurationReport,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum StandardsInspectReportStatus {
    Ok,
}

#[derive(Debug, Clone, Copy, Serialize)]
enum StandardsInspectReportCommand {
    #[serde(rename = "standards.inspect")]
    StandardsInspect,
}

#[derive(Debug, Clone)]
struct StandardsInspectReport {
    profile: String,
    standards_root: StandardsRoot,
    standards_count: usize,
    first_standard_name: String,
    first_standard_path: String,
    effective_configuration: StandardsInspectEffectiveConfigurationReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct StandardsInspectEffectiveConfigurationReport {
    profile: EffectiveConfigurationMetadata,
    standards_root: EffectiveConfigurationMetadata,
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
    /// Serializing success report as JSON failed.
    #[error("error: output_serialize_failed - serialize standards report as JSON: {source}")]
    ReportSerializeFailure {
        #[source]
        source: serde_json::Error,
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
    #[error("failed reading '{path}': {source}")]
    ReadFailure {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse project methodology config '{path}' as TOML: {source}")]
    ProjectMethodologyConfigParse {
        path: String,
        #[source]
        source: ConfigSecretsError,
    },
    #[error("project methodology config '{path}' is not compatible with this runtime: {source}")]
    ProjectMethodologyConfigIncompatible {
        path: String,
        #[source]
        source: ConfigSecretsError,
    },
    #[error("configured standards root '{path}' is invalid: {source}")]
    InvalidConfiguredStandardsRoot {
        path: String,
        #[source]
        source: InstallError,
    },
    #[error("path is not repository-relative: '{path}'")]
    NonRepositoryRelativePath { path: String },
    #[error("configured standards root is missing: '{path}'")]
    StandardsRootMissing { path: String },
    #[error("no standards markdown files found under configured standards root '{path}'")]
    NoStandardsFiles { path: String },
    #[error("failed to parse standards frontmatter in '{path}': {message}")]
    FrontmatterParse { path: String, message: String },
    #[error("standards tree walk exceeded maximum directory depth {limit} at '{path}'")]
    DirectoryDepthLimitExceeded { path: String, limit: usize },
    #[error("standards scan exceeded markdown file limit {limit} at '{path}'")]
    MarkdownFileLimitExceeded { path: String, limit: usize },
    #[error("standard markdown file exceeds byte limit {limit} in '{path}' ({actual} bytes)")]
    StandardFileTooLarge {
        path: String,
        limit: u64,
        actual: u64,
    },
    #[error(
        "standards scan exceeded total byte limit {limit} while reading '{path}' (total {actual} bytes)"
    )]
    StandardsTotalBytesLimitExceeded {
        path: String,
        limit: u64,
        actual: u64,
    },
    #[error(
        "standards frontmatter exceeds byte limit {limit} in '{path}' (at least {actual} bytes)"
    )]
    FrontmatterTooLarge {
        path: String,
        limit: usize,
        actual: usize,
    },
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
                source,
            },
        }
    })?;

    let config = ProjectMethodologyConfig::from_toml(&config_text).map_err(|source| {
        StandardsCommandError::ValidationFailed {
            source: StandardsError::ProjectMethodologyConfigParse {
                path: PROJECT_METHODOLOGY_CONFIG_REPO_PATH.to_owned(),
                source,
            },
        }
    })?;
    validate_project_methodology_config_schema(&config)?;

    let standards_root_relative = config.standards_root.as_str().to_owned();
    let standards_root =
        resolve_repo_relative_path(&repository_root, config.standards_root.as_str()).map_err(
            |source| StandardsCommandError::ValidationFailed {
                source: StandardsError::InvalidConfiguredStandardsRoot {
                    path: standards_root_relative.clone(),
                    source,
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

    let scan_summary = scan_standards(&repository_root, &standards_root)?;

    let (Some(first_standard_name), Some(first_standard_path)) = (
        scan_summary.first_standard_name,
        scan_summary.first_standard_path,
    ) else {
        return Err(StandardsCommandError::StandardsMissing {
            source: StandardsError::NoStandardsFiles {
                path: config.standards_root.as_str().to_owned(),
            },
        });
    };

    Ok(StandardsInspectReport {
        profile: config.profile.as_str().to_owned(),
        standards_root: config.standards_root,
        standards_count: scan_summary.standards_count,
        first_standard_name,
        first_standard_path,
        effective_configuration: StandardsInspectEffectiveConfigurationReport {
            profile: EffectiveConfigurationMetadata::project_explicit(
                EffectiveConfigurationSettingFamily::StandardsProfile,
            ),
            standards_root: EffectiveConfigurationMetadata::project_explicit(
                EffectiveConfigurationSettingFamily::StandardsRoot,
            ),
        },
    })
}

fn validate_project_methodology_config_schema(
    config: &ProjectMethodologyConfig,
) -> Result<(), StandardsCommandError> {
    config.schema_version.ensure_supported().map_err(|source| {
        StandardsCommandError::ValidationFailed {
            source: StandardsError::ProjectMethodologyConfigIncompatible {
                path: PROJECT_METHODOLOGY_CONFIG_REPO_PATH.to_owned(),
                source,
            },
        }
    })
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
