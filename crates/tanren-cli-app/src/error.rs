//! Top-level CLI app error boundary.

use thiserror::Error;

use crate::install::{InstallCommandError, InstallDriftCommandError};

/// Typed error boundary for `tanren-cli-app` command dispatch.
#[derive(Debug, Error)]
#[non_exhaustive]
pub(crate) enum CliAppError {
    /// Runtime or startup errors from non-install commands.
    #[error(transparent)]
    Runtime(#[from] anyhow::Error),
    /// `tanren-cli install` command failures.
    #[error(transparent)]
    Install(#[from] InstallCommandError),
    /// `tanren-cli drift` command failures.
    #[error(transparent)]
    Drift(#[from] InstallDriftCommandError),
}
