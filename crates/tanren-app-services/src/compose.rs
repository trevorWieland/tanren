//! Composition root — constructs the full service stack.
//!
//! Interface binaries call this module to get a ready-to-use
//! [`DispatchService`] without directly depending on store, policy,
//! or orchestrator crates.

use std::future::Future;
use std::sync::Arc;

use tanren_orchestrator::Orchestrator;
use tanren_policy::PolicyEngine;
use tanren_store::{ReplayPurgeConfig, ReplayPurgeService, ReplayPurgeStats, Store, StoreError};
use tokio::task::JoinHandle;

use crate::DispatchService;

/// Concrete service type for use by interface binaries.
pub type Service = DispatchService<Store>;

/// Open the persistent store for read commands without mutating schema.
pub(crate) async fn open_store_for_read(database_url: &str) -> Result<Store, StoreError> {
    Store::new(database_url).await
}

/// Open the persistent store for write commands and apply migrations.
pub(crate) async fn open_store_for_write(database_url: &str) -> Result<Store, StoreError> {
    Store::open_and_migrate(database_url).await
}

/// Run all pending schema migrations explicitly.
pub async fn run_migrations(database_url: &str) -> Result<(), StoreError> {
    let store = Store::new(database_url).await?;
    store.run_migrations().await
}

/// Build the policy engine used by the dispatch service stack.
#[must_use]
pub(crate) fn build_policy_engine() -> PolicyEngine {
    PolicyEngine::new()
}

/// Build an orchestrator from a store and policy engine.
#[must_use]
pub(crate) fn build_orchestrator(store: Store, policy: PolicyEngine) -> Orchestrator<Store> {
    Orchestrator::new(store, policy)
}

/// Build a dispatch service from an orchestrator.
#[must_use]
pub(crate) fn build_dispatch_service(orchestrator: Orchestrator<Store>) -> Service {
    DispatchService::new(orchestrator)
}

/// Build a fully-wired [`DispatchService`] for read commands.
///
/// Opens the store without schema mutation, validates schema readiness,
/// then wires policy and orchestrator.
pub async fn build_dispatch_service_for_read(database_url: &str) -> Result<Service, StoreError> {
    let store = open_store_for_read(database_url).await?;
    store.assert_schema_ready().await?;
    let policy = build_policy_engine();
    let orchestrator = build_orchestrator(store, policy);
    Ok(build_dispatch_service(orchestrator))
}

/// Build a fully-wired [`DispatchService`] for write commands.
///
/// Opens the store with migrate-before-write semantics and wires policy.
pub async fn build_dispatch_service_for_write(database_url: &str) -> Result<Service, StoreError> {
    let store = open_store_for_write(database_url).await?;
    let policy = build_policy_engine();
    let orchestrator = build_orchestrator(store, policy);
    Ok(build_dispatch_service(orchestrator))
}

/// Run a single bounded replay-ledger purge cycle and return its
/// stats.
///
/// Thin composition-root wrapper over
/// [`ReplayPurgeService::run_once`]. The CLI's
/// `tanren db purge-replay` subcommand is the canonical caller.
pub async fn purge_replay_tokens_once(
    database_url: &str,
    cfg: ReplayPurgeConfig,
) -> Result<ReplayPurgeStats, StoreError> {
    let store = Arc::new(open_store_for_read(database_url).await?);
    store.assert_schema_ready().await?;
    let service = ReplayPurgeService::new(store, cfg);
    service.run_once().await
}

/// Build a fully-wired [`crate::methodology::MethodologyService`]
/// over the persistent store. Applies migrations first so downstream
/// event appends never race a cold database.
///
/// Used by both `tanren-cli` methodology subcommands and
/// `tanren-mcp` so both transports produce byte-identical event
/// trails against the same store.
///
/// # Errors
/// Returns [`StoreError`] on open/migration failure.
pub async fn build_methodology_service(
    database_url: &str,
) -> Result<crate::methodology::MethodologyService, StoreError> {
    let store = open_store_for_write(database_url).await?;
    Ok(crate::methodology::MethodologyService::new(Arc::new(store)))
}

/// Spawn a long-running replay-purge loop onto the current tokio
/// runtime. Intended for the future `tanren-daemon` binary.
pub fn spawn_replay_purge(
    store: Arc<Store>,
    cfg: ReplayPurgeConfig,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> JoinHandle<()> {
    let service = ReplayPurgeService::new(store, cfg);
    tanren_store::spawn_replay_purge(service, shutdown)
}
