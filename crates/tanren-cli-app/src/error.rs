//! Top-level CLI app error boundary.

use tanren_app_services::AppServiceError;
use tanren_identity_policy::ValidationError;
use thiserror::Error;

use crate::install::{InstallCommandError, InstallDriftCommandError};

/// Typed error boundary for `tanren-cli-app` command dispatch.
#[derive(Debug, Error)]
#[non_exhaustive]
pub(crate) enum CliAppError {
    /// Writing a command report to stdout failed.
    #[error("error: internal_error — write {target} to stdout: {source}")]
    StdoutWriteFailure {
        target: &'static str,
        #[source]
        source: std::io::Error,
    },
    /// Constructing a command-local Tokio runtime failed.
    #[error("error: internal_error — build tokio runtime: {source}")]
    RuntimeBuildFailure {
        #[source]
        source: std::io::Error,
    },
    /// Running pending database migrations failed.
    #[error("error: internal_error — apply pending migrations: {source}")]
    MigrationFailure {
        #[source]
        source: AppServiceError,
    },
    /// Parsing `--identifier` as an email failed.
    #[error("error: validation_failed — parse --identifier as email: {source}")]
    IdentifierParseFailure {
        #[source]
        source: ValidationError,
    },
    /// Parsing `--invitation` as an invitation token failed.
    #[error("error: validation_failed — parse --invitation as invitation token: {source}")]
    InvitationTokenParseFailure {
        #[source]
        source: ValidationError,
    },
    /// Persisting the CLI session token failed while creating its parent directory.
    #[error("error: internal_error — create session dir {path}: {source}")]
    SessionDirectoryCreateFailure {
        path: String,
        #[source]
        source: std::io::Error,
    },
    /// Persisting the CLI session token failed while writing the session file.
    #[error("error: internal_error — write session to {path}: {source}")]
    SessionWriteFailure {
        path: String,
        #[source]
        source: std::io::Error,
    },
    /// Account command handler failed with a mapped CLI-facing taxonomy code.
    #[error("error: {code} — {summary}")]
    AccountServiceFailure {
        code: &'static str,
        summary: String,
        #[source]
        source: AppServiceError,
    },
    /// `tanren-cli install` command failures.
    #[error(transparent)]
    Install(#[from] InstallCommandError),
    /// `tanren-cli drift` command failures.
    #[error(transparent)]
    Drift(#[from] InstallDriftCommandError),
}

impl CliAppError {
    /// Convert an app-service failure into the stable CLI stderr shape:
    /// `error: <code> — <summary>`.
    #[must_use]
    pub(crate) fn from_account_service_error(source: AppServiceError) -> Self {
        let (code, summary) = match &source {
            AppServiceError::Account(reason) => (reason.code(), reason.summary().to_owned()),
            AppServiceError::InvalidInput(message) => ("validation_failed", message.clone()),
            AppServiceError::Store(err) => ("internal_error", err.to_string()),
            _ => ("internal_error", "unknown app-service failure".to_owned()),
        };
        Self::AccountServiceFailure {
            code,
            summary,
            source,
        }
    }
}
