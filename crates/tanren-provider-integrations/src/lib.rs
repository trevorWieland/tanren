//! Provider Integrations subsystem.
//!
//! Owns *outbound* connections to external systems: source control (GitHub,
//! GitLab), CI (GitHub Actions, Buildkite), issue trackers (Linear, Jira),
//! cloud or VM providers (Hetzner, GCP, AWS), and notification channels
//! (Slack, email). Concrete adapters live in separate crates introduced by
//! the slice that first needs each provider family.

mod source_control_fixture_env;

use std::sync::Arc;
use tanren_identity_policy::{AccountId, DesignatedHost, ProviderFamily, RepositoryRef};
use thiserror::Error;

#[cfg(any(test, feature = "test-hooks"))]
use std::collections::HashSet;
#[cfg(any(test, feature = "test-hooks"))]
use std::sync::Mutex;

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
#[non_exhaustive]
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
}

const SOURCE_CONTROL_PROVIDER_FIXTURE_ENV: &str = "TANREN_SOURCE_CONTROL_PROVIDER_FIXTURE";
const SOURCE_CONTROL_PROVIDER_ALLOW_ALL_FIXTURE: &str = "allow_all";

/// Deterministic source-control provider that always fails closed.
#[derive(Debug, Clone, Default)]
pub struct UnavailableSourceControlProvider;

#[async_trait::async_trait]
impl SourceControlProvider for UnavailableSourceControlProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::source_control()
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
}

/// Build the production source-control provider wiring.
///
/// This slice does not yet ship concrete provider adapters. Production
/// constructors therefore inject a fail-closed adapter that returns
/// `SourceControlError::ProviderUnavailable` for every operation unless
/// an explicit override is requested.
#[must_use]
pub fn production_source_control_provider() -> Arc<dyn SourceControlProvider> {
    if let Ok(raw) = std::env::var(SOURCE_CONTROL_PROVIDER_FIXTURE_ENV) {
        if raw
            .trim()
            .eq_ignore_ascii_case(SOURCE_CONTROL_PROVIDER_ALLOW_ALL_FIXTURE)
        {
            return fixture_allow_all_source_control_provider();
        }
        if let Some(provider) =
            source_control_fixture_env::env_fixture_source_control_provider(raw.trim())
        {
            return provider;
        }
    }
    Arc::new(UnavailableSourceControlProvider)
}

/// Deterministic allow-all provider used only by test and fixture paths.
#[derive(Debug, Clone, Default)]
pub struct AllowAllSourceControlProvider;

#[async_trait::async_trait]
impl SourceControlProvider for AllowAllSourceControlProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::source_control()
    }

    async fn ensure_provider_reachable(&self) -> Result<(), SourceControlError> {
        Ok(())
    }

    async fn ensure_host_reachable(
        &self,
        _host: &DesignatedHost,
    ) -> Result<(), SourceControlError> {
        Ok(())
    }

    async fn can_access_repository(
        &self,
        _actor_account_id: AccountId,
        _repository: &RepositoryRef,
    ) -> Result<bool, SourceControlError> {
        Ok(true)
    }

    async fn can_create_repository_at_host(
        &self,
        _actor_account_id: AccountId,
        _host: &DesignatedHost,
    ) -> Result<bool, SourceControlError> {
        Ok(true)
    }

    async fn create_repository(
        &self,
        _actor_account_id: AccountId,
        _host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<RepositoryRef, SourceControlError> {
        Ok(repository.clone())
    }
}

/// Deterministic allow-all constructor for controlled environments.
#[must_use]
pub fn fixture_allow_all_source_control_provider() -> Arc<dyn SourceControlProvider> {
    Arc::new(AllowAllSourceControlProvider)
}

/// Configuration for deterministic fixture SCM behavior.
#[cfg(any(test, feature = "test-hooks"))]
#[derive(Debug, Clone)]
pub struct FixtureSourceControlConfig {
    /// Whether provider-level connectivity checks succeed.
    pub provider_reachable: bool,
    /// Reachable hosts.
    pub reachable_hosts: HashSet<String>,
    /// `(actor, repository)` pairs allowed for repository-access checks.
    pub repository_access: HashSet<(AccountId, RepositoryRef)>,
    /// `(actor, host)` pairs allowed for create-on-host checks.
    pub host_create_access: HashSet<(AccountId, String)>,
    /// Whether repository creation calls fail with operation-failed.
    pub fail_repository_create: bool,
}

#[cfg(any(test, feature = "test-hooks"))]
impl Default for FixtureSourceControlConfig {
    fn default() -> Self {
        Self {
            provider_reachable: true,
            reachable_hosts: HashSet::new(),
            repository_access: HashSet::new(),
            host_create_access: HashSet::new(),
            fail_repository_create: false,
        }
    }
}

#[cfg(any(test, feature = "test-hooks"))]
#[derive(Debug)]
struct FixtureState {
    config: FixtureSourceControlConfig,
    created_repositories: HashSet<(String, RepositoryRef)>,
}

/// Deterministic in-memory source-control fixture for BDD and handler tests.
#[cfg(any(test, feature = "test-hooks"))]
#[derive(Debug, Clone)]
pub struct FixtureSourceControlProvider {
    state: Arc<Mutex<FixtureState>>,
}

#[cfg(any(test, feature = "test-hooks"))]
impl FixtureSourceControlProvider {
    /// Construct a fixture provider from explicit deterministic config.
    #[must_use]
    pub fn new(config: FixtureSourceControlConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(FixtureState {
                config,
                created_repositories: HashSet::new(),
            })),
        }
    }

    /// Snapshot created repositories as `(host, repository_ref)` tuples.
    #[must_use]
    pub fn created_repositories(&self) -> Vec<(String, RepositoryRef)> {
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut out: Vec<(String, RepositoryRef)> =
            guard.created_repositories.iter().cloned().collect();
        out.sort_by(|(left_host, left_repo), (right_host, right_repo)| {
            (left_host.as_str(), left_repo.as_str())
                .cmp(&(right_host.as_str(), right_repo.as_str()))
        });
        out
    }

    /// Update provider-reachability for this fixture.
    pub fn set_provider_reachable(&self, reachable: bool) {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.config.provider_reachable = reachable;
    }

    /// Toggle designated-host reachability for this fixture.
    pub fn set_host_reachable(&self, host: &DesignatedHost, reachable: bool) {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if reachable {
            guard
                .config
                .reachable_hosts
                .insert(host.as_str().to_owned());
        } else {
            guard.config.reachable_hosts.remove(host.as_str());
        }
    }

    /// Toggle repository-access permission for `(actor, repository)`.
    pub fn set_repository_access(
        &self,
        actor_account_id: AccountId,
        repository: RepositoryRef,
        allowed: bool,
    ) {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let pair = (actor_account_id, repository);
        if allowed {
            guard.config.repository_access.insert(pair);
        } else {
            guard.config.repository_access.remove(&pair);
        }
    }

    /// Toggle host-create permission for `(actor, host)`.
    pub fn set_host_create_access(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
        allowed: bool,
    ) {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let pair = (actor_account_id, host.as_str().to_owned());
        if allowed {
            guard.config.host_create_access.insert(pair);
        } else {
            guard.config.host_create_access.remove(&pair);
        }
    }

    /// Return whether this fixture observed a repository created at host.
    #[must_use]
    pub fn repository_created_at_host(
        &self,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> bool {
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard
            .created_repositories
            .contains(&(host.as_str().to_owned(), repository.clone()))
    }
}

#[cfg(any(test, feature = "test-hooks"))]
#[async_trait::async_trait]
impl SourceControlProvider for FixtureSourceControlProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::source_control()
    }

    async fn ensure_provider_reachable(&self) -> Result<(), SourceControlError> {
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.config.provider_reachable {
            Ok(())
        } else {
            Err(SourceControlError::ProviderUnreachable)
        }
    }

    async fn ensure_host_reachable(&self, host: &DesignatedHost) -> Result<(), SourceControlError> {
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.config.reachable_hosts.contains(host.as_str()) {
            Ok(())
        } else {
            Err(SourceControlError::HostUnreachable)
        }
    }

    async fn can_access_repository(
        &self,
        actor_account_id: AccountId,
        repository: &RepositoryRef,
    ) -> Result<bool, SourceControlError> {
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        Ok(guard
            .config
            .repository_access
            .contains(&(actor_account_id, repository.clone())))
    }

    async fn can_create_repository_at_host(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
    ) -> Result<bool, SourceControlError> {
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        Ok(guard
            .config
            .host_create_access
            .contains(&(actor_account_id, host.as_str().to_owned())))
    }

    async fn create_repository(
        &self,
        _actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<RepositoryRef, SourceControlError> {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.config.fail_repository_create {
            return Err(SourceControlError::OperationFailed);
        }
        guard
            .created_repositories
            .insert((host.as_str().to_owned(), repository.clone()));
        Ok(repository.clone())
    }
}
