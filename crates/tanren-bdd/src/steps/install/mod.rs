//! Install-flow step definitions for B-0068 / B-0070.
//!
//! The steps execute the real `tanren-cli` binary against a per-scenario
//! temporary repository fixture. No installer internals are called directly.

mod assertions;
mod assertions_uninstall;
mod context;
mod manifest_helpers;
mod repo_fixture;

pub(crate) use crate::steps::install_error::{InstallStepError, InstallStepResult};
pub(crate) use context::InstallContext;
pub(crate) use manifest_helpers::{RepositoryRelativePath, read_workspace_catalog_file};
