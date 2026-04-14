//! Composition root — constructs the full service stack.
//!
//! Interface binaries call this module to get a ready-to-use
//! [`DispatchService`] without directly depending on store, policy,
//! or orchestrator crates.

use tanren_orchestrator::Orchestrator;
use tanren_policy::PolicyEngine;
use tanren_store::{Store, StoreError};

use crate::DispatchService;

/// Concrete service type for use by interface binaries.
pub type Service = DispatchService<Store>;

/// Build a fully-wired [`DispatchService`] from a database URL.
///
/// Opens the store, runs migrations, creates the policy engine and
/// orchestrator, and returns the ready-to-use service.
pub async fn build_dispatch_service(database_url: &str) -> Result<Service, StoreError> {
    let store = Store::open_and_migrate(database_url).await?;
    let policy = PolicyEngine::new();
    let orchestrator = Orchestrator::new(store, policy);
    Ok(DispatchService::new(orchestrator))
}
