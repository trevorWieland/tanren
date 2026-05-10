//! Provider Integrations subsystem.
//!
//! Owns *outbound* connections to external systems: source control (GitHub,
//! GitLab), CI (GitHub Actions, Buildkite), issue trackers (Linear, Jira),
//! cloud or VM providers (Hetzner, GCP, AWS), and notification channels
//! (Slack, email). Concrete adapters live in separate crates introduced by
//! the slice that first needs each provider family.

#[cfg(any(test, feature = "test-hooks"))]
mod source_control_fixture;
#[cfg(any(test, feature = "test-hooks"))]
mod source_control_fixture_env;
mod source_control_registry;

use source_control_registry::SourceControlProviderRegistry;
use std::sync::Arc;
use tanren_identity_policy::{AccountId, DesignatedHost, ProviderFamily, RepositoryRef};
use thiserror::Error;

#[cfg(any(test, feature = "test-hooks"))]
pub use source_control_fixture::{
    AllowAllSourceControlProvider, FixtureEnvParseError, FixtureSourceControlConfig,
    FixtureSourceControlProvider, SourceControlCallCounters,
    fixture_allow_all_source_control_provider, fixture_source_control_provider_from_env,
    fixture_source_control_provider_from_env_value,
};

/// The outbound provider trait every adapter implements.
#[async_trait::async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// Family this adapter implements.
    fn family(&self) -> ProviderFamily;
}

/// Errors raised by provider integrations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProviderError {
    /// The provider call could not complete.
    #[error("provider call failed: {0}")]
    Call(String),
}

/// Failure taxonomy for source-control provider operations used by
/// project-setup commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SourceControlError {
    /// No source-control adapter is configured for this environment.
    #[error("source-control provider unavailable")]
    ProviderUnavailable,
    /// The provider connection itself is not reachable.
    #[error("source-control provider unreachable")]
    ProviderUnreachable,
    /// The designated source-control host is not reachable.
    #[error("designated source-control host unreachable")]
    HostUnreachable,
    /// The actor does not have permission for the requested operation.
    #[error("source-control operation unauthorized")]
    Unauthorized,
    /// The provider rejected the operation due to rate limiting.
    #[error("source-control provider rate-limited")]
    RateLimited,
    /// The provider rejected the operation because of a conflict.
    #[error("source-control provider conflict")]
    Conflict,
    /// Provider call failed for a non-reachability reason.
    #[error("source-control provider operation failed")]
    OperationFailed,
}

/// Provider-specific rate-limit status surfaced by preflight checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceControlRateLimitStatus {
    /// No provider-side rate limit currently blocks this action.
    NotLimited,
    /// Provider-side rate limiting currently blocks this action.
    RateLimited,
}

/// Typed remote repository identity metadata persisted as canonical Tanren
/// fields (never raw provider payload blobs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceControlRemoteIdentity {
    /// Source-control provider family that owns this remote identity.
    pub provider_family: ProviderFamily,
    /// Designated host resolved for this repository.
    pub designated_host: DesignatedHost,
    /// Canonical repository identity (`owner/name`).
    pub repository: RepositoryRef,
    /// Stable provider remote id/token for this repository.
    pub provider_remote_id: String,
    /// Optional stable provider URL for this repository.
    pub provider_remote_url: Option<String>,
}

/// Shared provider preflight reachability output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceControlReachabilityStatus {
    /// Whether the provider endpoint is reachable.
    pub provider_reachable: bool,
    /// Whether the designated host endpoint is reachable.
    pub host_reachable: bool,
}

/// Batched preflight for "connect existing repository" commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceControlConnectPreflight {
    /// Reachability state at provider + designated-host layers.
    pub reachability: SourceControlReachabilityStatus,
    /// Whether the actor can access the repository.
    pub repository_access: bool,
    /// Provider-side rate-limit status for this action.
    pub rate_limit: SourceControlRateLimitStatus,
    /// Safe remote repository identity metadata.
    pub remote_identity: SourceControlRemoteIdentity,
}

/// Batched preflight for "create new repository" commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceControlCreatePreflight {
    /// Reachability state at provider + designated-host layers.
    pub reachability: SourceControlReachabilityStatus,
    /// Whether the actor can create repositories at this host.
    pub host_create_access: bool,
    /// Provider-side rate-limit status for this action.
    pub rate_limit: SourceControlRateLimitStatus,
    /// Safe remote repository identity metadata for the repository to create.
    pub remote_identity: SourceControlRemoteIdentity,
}

/// Source-control provider port consumed by `tanren-app-services`.
///
/// Real adapters (GitHub/GitLab/local host) and deterministic test fixtures
/// implement this trait.
#[async_trait::async_trait]
pub trait SourceControlProvider: Send + Sync + std::fmt::Debug {
    /// Source-control provider family identifier.
    fn family(&self) -> ProviderFamily;

    /// Execute batched preflight checks for a connect-existing-repository
    /// operation.
    async fn preflight_connect_repository(
        &self,
        actor_account_id: AccountId,
        repository: &RepositoryRef,
    ) -> Result<SourceControlConnectPreflight, SourceControlError>;

    /// Execute batched preflight checks for a create-new-repository operation.
    async fn preflight_create_repository(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<SourceControlCreatePreflight, SourceControlError>;

    /// Create a repository and return its canonical identity.
    async fn create_repository(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<RepositoryRef, SourceControlError>;

    /// Delete a previously created repository.
    async fn delete_repository(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<(), SourceControlError>;
}

const DEFAULT_SOURCE_CONTROL_BINDING_HOST: &str = "source-control.local";

fn default_source_control_binding_host() -> Result<DesignatedHost, SourceControlError> {
    DesignatedHost::parse(DEFAULT_SOURCE_CONTROL_BINDING_HOST)
        .map_err(|_| SourceControlError::OperationFailed)
}

/// Deterministic source-control provider that always fails closed.
#[derive(Debug, Clone, Default)]
pub struct UnavailableSourceControlProvider;

#[async_trait::async_trait]
impl SourceControlProvider for UnavailableSourceControlProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::source_control()
    }

    async fn preflight_connect_repository(
        &self,
        _actor_account_id: AccountId,
        _repository: &RepositoryRef,
    ) -> Result<SourceControlConnectPreflight, SourceControlError> {
        Err(SourceControlError::ProviderUnavailable)
    }

    async fn preflight_create_repository(
        &self,
        _actor_account_id: AccountId,
        _host: &DesignatedHost,
        _repository: &RepositoryRef,
    ) -> Result<SourceControlCreatePreflight, SourceControlError> {
        Err(SourceControlError::ProviderUnavailable)
    }

    async fn create_repository(
        &self,
        _actor_account_id: AccountId,
        _host: &DesignatedHost,
        _repository: &RepositoryRef,
    ) -> Result<RepositoryRef, SourceControlError> {
        Err(SourceControlError::ProviderUnavailable)
    }

    async fn delete_repository(
        &self,
        _actor_account_id: AccountId,
        _host: &DesignatedHost,
        _repository: &RepositoryRef,
    ) -> Result<(), SourceControlError> {
        Err(SourceControlError::ProviderUnavailable)
    }
}

/// Build the production source-control provider wiring.
///
/// This slice does not yet ship concrete provider adapters. Production
/// constructors therefore inject a fail-closed adapter that returns
/// `SourceControlError::ProviderUnavailable` for every operation until a
/// concrete adapter is registered.
#[must_use]
pub fn production_source_control_provider() -> Arc<dyn SourceControlProvider> {
    SourceControlProviderRegistry::production().resolve()
}
