use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use tanren_identity_policy::{AccountId, DesignatedHost, ProviderFamily, RepositoryRef};
use uuid::Uuid;

use crate::{
    SourceControlConnectPreflight, SourceControlCreatePreflight, SourceControlError,
    SourceControlProvider, SourceControlRateLimitStatus, SourceControlReachabilityStatus,
    SourceControlRemoteIdentity,
};

#[derive(Debug, Clone, Default)]
struct EnvFixtureSourceControlConfig {
    preflight_mode: EnvFixturePreflightMode,
    reachable_hosts: HashSet<String>,
    repository_access: HashSet<(AccountId, RepositoryRef)>,
    host_create_access: HashSet<(AccountId, String)>,
    fail_repository_create: bool,
    fail_repository_delete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum EnvFixturePreflightMode {
    #[default]
    Ready,
    ProviderUnreachable,
    RateLimited,
}

#[derive(Debug, Clone)]
struct EnvFixtureSourceControlProvider {
    config: EnvFixtureSourceControlConfig,
    created_repositories: Arc<Mutex<HashSet<(String, RepositoryRef)>>>,
}

#[async_trait::async_trait]
impl SourceControlProvider for EnvFixtureSourceControlProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::source_control()
    }

    async fn preflight_connect_repository(
        &self,
        actor_account_id: AccountId,
        repository: &RepositoryRef,
    ) -> Result<SourceControlConnectPreflight, SourceControlError> {
        match self.config.preflight_mode {
            EnvFixturePreflightMode::Ready => {}
            EnvFixturePreflightMode::ProviderUnreachable => {
                return Err(SourceControlError::ProviderUnreachable);
            }
            EnvFixturePreflightMode::RateLimited => {
                return Err(SourceControlError::RateLimited);
            }
        }
        let designated_host = DesignatedHost::parse("source-control.local")
            .map_err(|_| SourceControlError::OperationFailed)?;
        Ok(SourceControlConnectPreflight {
            reachability: SourceControlReachabilityStatus {
                provider_reachable: true,
                host_reachable: true,
            },
            repository_access: self
                .config
                .repository_access
                .contains(&(actor_account_id, repository.clone())),
            rate_limit: SourceControlRateLimitStatus::NotLimited,
            remote_identity: env_fixture_remote_identity(
                self.family(),
                &designated_host,
                repository,
            ),
        })
    }

    async fn preflight_create_repository(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<SourceControlCreatePreflight, SourceControlError> {
        match self.config.preflight_mode {
            EnvFixturePreflightMode::Ready => {}
            EnvFixturePreflightMode::ProviderUnreachable => {
                return Err(SourceControlError::ProviderUnreachable);
            }
            EnvFixturePreflightMode::RateLimited => {
                return Err(SourceControlError::RateLimited);
            }
        }
        if !self.config.reachable_hosts.contains(host.as_str()) {
            return Err(SourceControlError::HostUnreachable);
        }
        Ok(SourceControlCreatePreflight {
            reachability: SourceControlReachabilityStatus {
                provider_reachable: true,
                host_reachable: true,
            },
            host_create_access: self
                .config
                .host_create_access
                .contains(&(actor_account_id, host.as_str().to_owned())),
            rate_limit: SourceControlRateLimitStatus::NotLimited,
            remote_identity: env_fixture_remote_identity(self.family(), host, repository),
        })
    }

    async fn create_repository(
        &self,
        _actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<RepositoryRef, SourceControlError> {
        if self.config.fail_repository_create {
            return Err(SourceControlError::OperationFailed);
        }
        let mut guard = match self.created_repositories.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.insert((host.as_str().to_owned(), repository.clone()));
        Ok(repository.clone())
    }

    async fn delete_repository(
        &self,
        _actor_account_id: AccountId,
        host: &DesignatedHost,
        repository: &RepositoryRef,
    ) -> Result<(), SourceControlError> {
        if self.config.fail_repository_delete {
            return Err(SourceControlError::OperationFailed);
        }
        let mut guard = match self.created_repositories.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.remove(&(host.as_str().to_owned(), repository.clone()));
        Ok(())
    }
}

pub(crate) fn env_fixture_source_control_provider(
    raw: &str,
) -> Option<Arc<dyn SourceControlProvider>> {
    const PREFIX: &str = "fixture-v1;";
    if !raw.starts_with(PREFIX) {
        return None;
    }
    let mut config = EnvFixtureSourceControlConfig::default();
    let mut provider_reachable = true;
    let mut rate_limited = false;
    for entry in raw[PREFIX.len()..].split(';') {
        if entry.is_empty() {
            continue;
        }
        let Some((key, value)) = entry.split_once('=') else {
            continue;
        };
        match key {
            "provider_reachable" => {
                provider_reachable = parse_bool_flag(value, true);
            }
            "rate_limited" => {
                rate_limited = parse_bool_flag(value, false);
            }
            "fail_repository_create" => {
                config.fail_repository_create = parse_bool_flag(value, false);
            }
            "fail_repository_delete" => {
                config.fail_repository_delete = parse_bool_flag(value, false);
            }
            "reachable_hosts" => {
                for host in value.split('|').filter(|token| !token.trim().is_empty()) {
                    config
                        .reachable_hosts
                        .insert(host.trim().to_ascii_lowercase());
                }
            }
            "repo_access" => {
                for raw_pair in value.split('|').filter(|token| !token.trim().is_empty()) {
                    if let Some(pair) = parse_repo_access_pair(raw_pair.trim()) {
                        config.repository_access.insert(pair);
                    }
                }
            }
            "host_create_access" => {
                for raw_pair in value.split('|').filter(|token| !token.trim().is_empty()) {
                    if let Some(pair) = parse_host_access_pair(raw_pair.trim()) {
                        config.host_create_access.insert(pair);
                    }
                }
            }
            _ => {}
        }
    }
    config.preflight_mode = if !provider_reachable {
        EnvFixturePreflightMode::ProviderUnreachable
    } else if rate_limited {
        EnvFixturePreflightMode::RateLimited
    } else {
        EnvFixturePreflightMode::Ready
    };
    Some(Arc::new(EnvFixtureSourceControlProvider {
        config,
        created_repositories: Arc::new(Mutex::new(HashSet::new())),
    }))
}

fn env_fixture_remote_identity(
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

fn parse_bool_flag(raw: &str, default: bool) -> bool {
    match raw.trim() {
        "1" | "true" | "TRUE" | "True" => true,
        "0" | "false" | "FALSE" | "False" => false,
        _ => default,
    }
}

fn parse_repo_access_pair(raw: &str) -> Option<(AccountId, RepositoryRef)> {
    let (account_raw, repository_raw) = raw.split_once('@')?;
    let account = parse_account_id(account_raw.trim())?;
    let repository = RepositoryRef::parse(repository_raw.trim()).ok()?;
    Some((account, repository))
}

fn parse_host_access_pair(raw: &str) -> Option<(AccountId, String)> {
    let (account_raw, host_raw) = raw.split_once('@')?;
    let account = parse_account_id(account_raw.trim())?;
    let host = host_raw.trim().to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    Some((account, host))
}

fn parse_account_id(raw: &str) -> Option<AccountId> {
    let uuid = Uuid::parse_str(raw).ok()?;
    Some(AccountId::new(uuid))
}
