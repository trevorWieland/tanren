//! Standards command error taxonomy.

use tanren_configuration_secrets::ConfigSecretsError;
use thiserror::Error;

use crate::install::InstallError;

/// Typed `tanren-cli standards` command failures at the CLI-library boundary.
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

impl StandardsCommandError {
    pub(super) fn validation_failed(source: StandardsError) -> Self {
        Self::ValidationFailed { source }
    }

    pub(super) fn standards_missing(source: StandardsError) -> Self {
        Self::StandardsMissing { source }
    }

    pub(super) fn standards_parse_failed(source: StandardsError) -> Self {
        Self::StandardsParseFailed { source }
    }

    pub(super) fn report_serialize_failure(source: serde_json::Error) -> Self {
        Self::ReportSerializeFailure { source }
    }

    pub(super) fn stdout_write_failure(source: std::io::Error) -> Self {
        Self::StdoutWriteFailure { source }
    }
}

/// Domain errors for standards configuration and scanning.
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
    #[error("failed to parse standards frontmatter in '{path}': {source}")]
    FrontmatterParse {
        path: String,
        #[source]
        source: StandardsFrontmatterError,
    },
    #[error("standards tree walk exceeded maximum directory depth {limit} at '{path}'")]
    DirectoryDepthLimitExceeded { path: String, limit: usize },
    #[error(
        "standards scan exceeded directory entry limit {limit} at '{path}' ({entries} entries traversed)"
    )]
    DirectoryEntryLimitExceeded {
        path: String,
        limit: usize,
        entries: usize,
    },
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

/// Typed standards frontmatter parsing failures.
#[derive(Debug, Error)]
pub(crate) enum StandardsFrontmatterError {
    #[error("missing opening frontmatter delimiter")]
    MissingOpeningDelimiter,
    #[error("missing closing frontmatter delimiter")]
    MissingClosingDelimiter,
    #[error("invalid frontmatter byte bounds")]
    InvalidByteBounds,
    #[error("frontmatter byte counting overflowed")]
    ByteCountingOverflow,
    #[error("invalid YAML frontmatter")]
    FrontmatterInvalid,
    #[error("frontmatter 'name' must not be empty")]
    EmptyName,
}
