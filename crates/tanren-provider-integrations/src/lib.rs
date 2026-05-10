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
    AllowAllSourceControlProvider, FixtureSourceControlConfig, FixtureSourceControlProvider,
    SourceControlCallCounters, fixture_allow_all_source_control_provider,
    fixture_source_control_provider_from_env, fixture_source_control_provider_from_env_value,
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
    /// Provider call failed for a non-reachability reason.
    #[error("source-control provider operation failed")]
    OperationFailed,
}

/// Source-control provider port consumed by `tanren-app-services`.
///
/// Real adapters (GitHub/GitLab/local host) and deterministic test fixtures
/// implement this trait.
#[async_trait::async_trait]
pub trait SourceControlProvider: Send + Sync + std::fmt::Debug {
    /// Source-control provider family identifier.
    fn family(&self) -> ProviderFamily;

    /// Resolve consumer-facing host metadata for a repository binding.
    fn host_for_repository_binding(
        &self,
        repository: &RepositoryRef,
    ) -> Result<DesignatedHost, SourceControlError>;

    /// Validate that the provider is reachable.
    async fn ensure_provider_reachable(&self) -> Result<(), SourceControlError>;

    /// Validate that the designated host is reachable.
    async fn ensure_host_reachable(&self, host: &DesignatedHost) -> Result<(), SourceControlError>;

    /// Validate that the actor can access the repository at the provider.
    async fn can_access_repository(
        &self,
        actor_account_id: AccountId,
        repository: &RepositoryRef,
    ) -> Result<bool, SourceControlError>;

    /// Validate that the actor can create repositories at the designated host.
    async fn can_create_repository_at_host(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
    ) -> Result<bool, SourceControlError>;

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

    fn host_for_repository_binding(
        &self,
        _repository: &RepositoryRef,
    ) -> Result<DesignatedHost, SourceControlError> {
        default_source_control_binding_host()
    }

    async fn ensure_provider_reachable(&self) -> Result<(), SourceControlError> {
        Err(SourceControlError::ProviderUnavailable)
    }

    async fn ensure_host_reachable(
        &self,
        _host: &DesignatedHost,
    ) -> Result<(), SourceControlError> {
        Err(SourceControlError::ProviderUnavailable)
    }

    async fn can_access_repository(
        &self,
        _actor_account_id: AccountId,
        _repository: &RepositoryRef,
    ) -> Result<bool, SourceControlError> {
        Err(SourceControlError::ProviderUnavailable)
    }

    async fn can_create_repository_at_host(
        &self,
        _actor_account_id: AccountId,
        _host: &DesignatedHost,
    ) -> Result<bool, SourceControlError> {
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
