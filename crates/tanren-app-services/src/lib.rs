//! Command and query handlers shared by every Tanren interface binary.
//!
//! Per architecture, equivalent operations across web/api/cli/mcp/tui must
//! resolve to the same handler — this crate is that seam. Interface binaries
//! depend on `tanren-app-services` (and `tanren-contract` for wire shapes);
//! they do not import domain, store, or runtime crates directly.

use serde::{Deserialize, Serialize};
use tanren_contract::ContractVersion;
use tanren_store::{Store, StoreError};
use thiserror::Error;

/// Stable response shape for the cross-interface health/liveness query.
///
/// Every interface (the api's `/health`, the cli's `--version`, the mcp
/// server's introspection, the tui's status bar, the daemon's startup log)
/// resolves to [`Handlers::health`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Static "ok" string. Present so consumers can match on a discriminator
    /// rather than HTTP status alone.
    pub status: &'static str,
    /// Build-time package version of the binary that produced the report.
    pub version: &'static str,
    /// Wire-contract version this binary speaks.
    pub contract_version: ContractVersion,
}

/// Stateless handler facade. Future versions of this crate will hold
/// dependencies (store, policy, runtime); F-0001 ships the seam itself.
#[derive(Debug, Default, Clone, Copy)]
pub struct Handlers;

impl Handlers {
    /// Construct a fresh handler facade.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Liveness query. Returns the same shape regardless of which interface
    /// invoked it.
    #[must_use]
    pub fn health(&self, version: &'static str) -> HealthReport {
        HealthReport {
            status: "ok",
            version,
            contract_version: ContractVersion::CURRENT,
        }
    }

    /// Apply all pending database migrations against the supplied URL.
    ///
    /// # Errors
    ///
    /// Returns [`AppServiceError::Store`] if connection or migration fails.
    pub async fn migrate(&self, database_url: &str) -> Result<(), AppServiceError> {
        let store = Store::connect(database_url).await?;
        store.migrate().await?;
        Ok(())
    }
}

/// Errors raised by app-service handlers.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppServiceError {
    /// A handler input failed validation.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// The underlying store layer raised an error.
    #[error(transparent)]
    Store(#[from] StoreError),
}
