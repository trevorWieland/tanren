use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use tanren_identity_policy::{AccountId, DesignatedHost, ProviderFamily, RepositoryRef};

use crate::{
    SourceControlConnectPreflight, SourceControlCreatePreflight, SourceControlError,
    SourceControlProvider, SourceControlRateLimitStatus, SourceControlReachabilityStatus,
    SourceControlRemoteIdentity, default_source_control_binding_host, source_control_fixture_env,
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

    async fn preflight_connect_repository(
        &self,
        _actor_account_id: AccountId,
        repository: &RepositoryRef,
    ) -> Result<SourceControlConnectPreflight, SourceControlError> {
        let designated_host = default_source_control_binding_host()?;
        Ok(SourceControlConnectPreflight {
            reachability: SourceControlReachabilityStatus {
                provider_reachable: true,
                host_reachable: true,
            },
            repository_access: true,
            rate_limit: SourceControlRateLimitStatus::NotLimited,
            remote_identity: fixture_remote_identity(self.family(), &designated_host, repository),
        })
    }

    async fn preflight_create_repository(
        &self,
        _actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<SourceControlCreatePreflight, SourceControlError> {
        let designated_host = host.clone();
        Ok(SourceControlCreatePreflight {
            reachability: SourceControlReachabilityStatus {
                provider_reachable: true,
                host_reachable: true,
            },
            host_create_access: true,
            rate_limit: SourceControlRateLimitStatus::NotLimited,
            remote_identity: fixture_remote_identity(self.family(), &designated_host, repository),
        })
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
    /// Provider preflight readiness mode.
    pub preflight_mode: FixturePreflightMode,
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
            preflight_mode: FixturePreflightMode::Ready,
            reachable_hosts: HashSet::new(),
            repository_access: HashSet::new(),
            host_create_access: HashSet::new(),
            fail_repository_create: false,
            fail_repository_delete: false,
        }
    }
}

/// Provider preflight mode used by the deterministic fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixturePreflightMode {
    /// Provider is reachable and not rate-limited.
    Ready,
    /// Provider endpoint is unreachable.
    ProviderUnreachable,
    /// Provider is reachable but currently rate-limited.
    RateLimited,
}

/// Source-control fixture call counters for assertion-heavy tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SourceControlCallCounters {
    /// Count of `preflight_connect_repository` calls.
    pub preflight_connect_repository: u64,
    /// Count of `preflight_create_repository` calls.
    pub preflight_create_repository: u64,
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
        if reachable {
            if matches!(
                guard.config.preflight_mode,
                FixturePreflightMode::ProviderUnreachable
            ) {
                guard.config.preflight_mode = FixturePreflightMode::Ready;
            }
        } else {
            guard.config.preflight_mode = FixturePreflightMode::ProviderUnreachable;
        }
    }

    /// Toggle provider rate-limit status for this fixture.
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn set_rate_limited(&self, limited: bool) {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if limited {
            guard.config.preflight_mode = FixturePreflightMode::RateLimited;
        } else if matches!(
            guard.config.preflight_mode,
            FixturePreflightMode::RateLimited
        ) {
            guard.config.preflight_mode = FixturePreflightMode::Ready;
        }
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

    async fn preflight_connect_repository(
        &self,
        actor_account_id: AccountId,
        repository: &RepositoryRef,
    ) -> Result<SourceControlConnectPreflight, SourceControlError> {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.counters.preflight_connect_repository += 1;
        match guard.config.preflight_mode {
            FixturePreflightMode::Ready => {}
            FixturePreflightMode::ProviderUnreachable => {
                return Err(SourceControlError::ProviderUnreachable);
            }
            FixturePreflightMode::RateLimited => {
                return Err(SourceControlError::RateLimited);
            }
        }
        let designated_host = default_source_control_binding_host()?;
        Ok(SourceControlConnectPreflight {
            reachability: SourceControlReachabilityStatus {
                provider_reachable: true,
                host_reachable: true,
            },
            repository_access: guard
                .config
                .repository_access
                .contains(&(actor_account_id, repository.clone())),
            rate_limit: SourceControlRateLimitStatus::NotLimited,
            remote_identity: fixture_remote_identity(self.family(), &designated_host, repository),
        })
    }

    async fn preflight_create_repository(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<SourceControlCreatePreflight, SourceControlError> {
        let mut guard = match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.counters.preflight_create_repository += 1;
        match guard.config.preflight_mode {
            FixturePreflightMode::Ready => {}
            FixturePreflightMode::ProviderUnreachable => {
                return Err(SourceControlError::ProviderUnreachable);
            }
            FixturePreflightMode::RateLimited => {
                return Err(SourceControlError::RateLimited);
            }
        }
        if !guard.config.reachable_hosts.contains(host.as_str()) {
            return Err(SourceControlError::HostUnreachable);
        }
        Ok(SourceControlCreatePreflight {
            reachability: SourceControlReachabilityStatus {
                provider_reachable: true,
                host_reachable: true,
            },
            host_create_access: guard
                .config
                .host_create_access
                .contains(&(actor_account_id, host.as_str().to_owned())),
            rate_limit: SourceControlRateLimitStatus::NotLimited,
            remote_identity: fixture_remote_identity(self.family(), host, repository),
        })
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

fn fixture_remote_identity(
    provider_family: ProviderFamily,
    designated_host: &DesignatedHost,
    repository: &RepositoryRef,
) -> SourceControlRemoteIdentity {
    SourceControlRemoteIdentity {
        provider_family,
        designated_host: designated_host.clone(),
        repository: repository.clone(),
        provider_remote_id: format!("fixture:{}:{repository}", designated_host.as_str()),
        provider_remote_url: Some(format!(
            "https://{}/{}",
            designated_host.as_str(),
            repository.as_str()
        )),
    }
}
