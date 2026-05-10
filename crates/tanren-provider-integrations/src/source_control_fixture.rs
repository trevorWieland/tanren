use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use tanren_identity_policy::{AccountId, DesignatedHost, ProviderFamily, RepositoryRef};

use crate::{
    SourceControlError, SourceControlProvider, default_source_control_binding_host,
    source_control_fixture_env,
};

const SOURCE_CONTROL_PROVIDER_FIXTURE_ENV: &str = "TANREN_SOURCE_CONTROL_PROVIDER_FIXTURE";
const SOURCE_CONTROL_PROVIDER_ALLOW_ALL_FIXTURE: &str = "allow_all";

/// Deterministic allow-all provider used only by test and fixture paths.
#[derive(Debug, Clone, Default)]
pub struct AllowAllSourceControlProvider;

#[async_trait::async_trait]
impl SourceControlProvider for AllowAllSourceControlProvider {
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

    async fn delete_repository(
        &self,
        _actor_account_id: AccountId,
        _host: &DesignatedHost,
        _repository: &RepositoryRef,
    ) -> Result<(), SourceControlError> {
        Ok(())
    }
}

/// Deterministic allow-all constructor for controlled environments.
#[must_use]
pub fn fixture_allow_all_source_control_provider() -> Arc<dyn SourceControlProvider> {
    Arc::new(AllowAllSourceControlProvider)
}

/// Build a fixture provider from the standard fixture environment variable.
#[must_use]
#[cfg(any(test, feature = "test-hooks"))]
pub fn fixture_source_control_provider_from_env() -> Option<Arc<dyn SourceControlProvider>> {
    let raw = std::env::var(SOURCE_CONTROL_PROVIDER_FIXTURE_ENV).ok()?;
    fixture_source_control_provider_from_env_value(raw.trim())
}

/// Build a fixture provider from a deterministic fixture env value.
#[must_use]
#[cfg(any(test, feature = "test-hooks"))]
pub fn fixture_source_control_provider_from_env_value(
    raw: &str,
) -> Option<Arc<dyn SourceControlProvider>> {
    if raw.eq_ignore_ascii_case(SOURCE_CONTROL_PROVIDER_ALLOW_ALL_FIXTURE) {
        return Some(fixture_allow_all_source_control_provider());
    }
    source_control_fixture_env::env_fixture_source_control_provider(raw)
}

/// Configuration for deterministic fixture SCM behavior.
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
    /// Whether repository delete calls fail with operation-failed.
    pub fail_repository_delete: bool,
}

impl Default for FixtureSourceControlConfig {
    fn default() -> Self {
        Self {
            provider_reachable: true,
            reachable_hosts: HashSet::new(),
            repository_access: HashSet::new(),
            host_create_access: HashSet::new(),
            fail_repository_create: false,
            fail_repository_delete: false,
        }
    }
}

/// Source-control fixture call counters for assertion-heavy tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceControlCallCounters {
    /// Count of `ensure_provider_reachable` calls.
    pub ensure_provider_reachable: u64,
    /// Count of `ensure_host_reachable` calls.
    pub ensure_host_reachable: u64,
    /// Count of `can_access_repository` calls.
    pub can_access_repository: u64,
    /// Count of `can_create_repository_at_host` calls.
    pub can_create_repository_at_host: u64,
    /// Count of `create_repository` calls.
    pub create_repository: u64,
    /// Count of `delete_repository` calls.
    pub delete_repository: u64,
}

#[derive(Debug)]
struct FixtureState {
    config: FixtureSourceControlConfig,
    created_repositories: HashSet<(String, RepositoryRef)>,
    counters: SourceControlCallCounters,
}

/// Deterministic in-memory source-control fixture for BDD and handler tests.
#[derive(Debug, Clone)]
pub struct FixtureSourceControlProvider {
    state: Arc<Mutex<FixtureState>>,
}

impl FixtureSourceControlProvider {
    /// Construct a fixture provider from explicit deterministic config.
    #[must_use]
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn new(config: FixtureSourceControlConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(FixtureState {
                config,
                created_repositories: HashSet::new(),
                counters: SourceControlCallCounters::default(),
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
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn set_provider_reachable(&self, reachable: bool) {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.config.provider_reachable = reachable;
    }

    /// Toggle designated-host reachability for this fixture.
    #[cfg(any(test, feature = "test-hooks"))]
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
    #[cfg(any(test, feature = "test-hooks"))]
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

    /// Snapshot source-control call counters.
    #[must_use]
    pub fn call_counters(&self) -> SourceControlCallCounters {
        let guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.counters
    }
}

#[async_trait::async_trait]
impl SourceControlProvider for FixtureSourceControlProvider {
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
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.counters.ensure_provider_reachable += 1;
        if guard.config.provider_reachable {
            Ok(())
        } else {
            Err(SourceControlError::ProviderUnreachable)
        }
    }

    async fn ensure_host_reachable(&self, host: &DesignatedHost) -> Result<(), SourceControlError> {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.counters.ensure_host_reachable += 1;
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
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.counters.can_access_repository += 1;
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
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.counters.can_create_repository_at_host += 1;
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
        guard.counters.create_repository += 1;
        if guard.config.fail_repository_create {
            return Err(SourceControlError::OperationFailed);
        }
        guard
            .created_repositories
            .insert((host.as_str().to_owned(), repository.clone()));
        Ok(repository.clone())
    }

    async fn delete_repository(
        &self,
        _actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<(), SourceControlError> {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.counters.delete_repository += 1;
        if guard.config.fail_repository_delete {
            return Err(SourceControlError::OperationFailed);
        }
        guard
            .created_repositories
            .remove(&(host.as_str().to_owned(), repository.clone()));
        Ok(())
    }
}
