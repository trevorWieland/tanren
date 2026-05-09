//! Provider Integrations subsystem.
//!
//! Owns *outbound* connections to external systems: source control (GitHub,
//! GitLab), CI (GitHub Actions, Buildkite), issue trackers (Linear, Jira),
//! cloud or VM providers (Hetzner, GCP, AWS), and notification channels
//! (Slack, email). Concrete adapters live in separate crates introduced by
//! the slice that first needs each provider family.

use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, RepositoryRef};
use thiserror::Error;

#[cfg(any(test, feature = "test-hooks"))]
use std::collections::HashSet;
#[cfg(any(test, feature = "test-hooks"))]
use std::sync::{Arc, Mutex};

/// Stable identifier for a provider family (`"github"`, `"linear"`, ...).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderFamily(String);

impl ProviderFamily {
    /// Wrap a provider family slug.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the family slug.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

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
    /// Validate that the provider is reachable.
    async fn ensure_provider_reachable(&self) -> Result<(), SourceControlError>;

    /// Validate that the designated host is reachable.
    async fn ensure_host_reachable(&self, host: &str) -> Result<(), SourceControlError>;

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
        host: &str,
    ) -> Result<bool, SourceControlError>;

    /// Create a repository and return its canonical identity.
    async fn create_repository(
        &self,
        actor_account_id: AccountId,
        host: &str,
        repository: &RepositoryRef,
    ) -> Result<RepositoryRef, SourceControlError>;
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
}

#[cfg(any(test, feature = "test-hooks"))]
#[async_trait::async_trait]
impl SourceControlProvider for FixtureSourceControlProvider {
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

    async fn ensure_host_reachable(&self, host: &str) -> Result<(), SourceControlError> {
        let trimmed = host.trim();
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.config.reachable_hosts.contains(trimmed) {
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
        host: &str,
    ) -> Result<bool, SourceControlError> {
        let host = host.trim().to_owned();
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        Ok(guard
            .config
            .host_create_access
            .contains(&(actor_account_id, host)))
    }

    async fn create_repository(
        &self,
        _actor_account_id: AccountId,
        host: &str,
        repository: &RepositoryRef,
    ) -> Result<RepositoryRef, SourceControlError> {
        let host = host.trim().to_owned();
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.config.fail_repository_create {
            return Err(SourceControlError::OperationFailed);
        }
        guard
            .created_repositories
            .insert((host, repository.clone()));
        Ok(repository.clone())
    }
}
