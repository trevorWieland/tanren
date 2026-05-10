//! CLI adapter layer for install and upgrade commands.
//!
//! This module provides CLI-specific argument parsing, stdout formatting,
//! and error wrapping. All shared install/upgrade planning, catalog,
//! manifest, and writer logic lives in `tanren-delivery`.

mod cli;
mod error;
mod upgrade;

pub use cli::InstallCommand;
pub use error::{InstallCommandError, UpgradeCommandError};
pub use upgrade::UpgradeCommand;

/// Re-export shared delivery types used by CLI adapters.
pub use tanren_delivery::install::InstallReport;

#[cfg(feature = "test-hooks")]
pub use tanren_delivery::install::RepoRelativePath;

#[cfg(feature = "test-hooks")]
pub use tanren_delivery::install::sha256_hex;
