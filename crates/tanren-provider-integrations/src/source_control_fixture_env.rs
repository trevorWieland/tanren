use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use tanren_identity_policy::{AccountId, DesignatedHost, ProviderFamily, RepositoryRef};
use uuid::Uuid;

use crate::{SourceControlError, SourceControlProvider};

#[derive(Debug, Clone, Default)]
struct EnvFixtureSourceControlConfig {
    provider_reachable: bool,
    reachable_hosts: HashSet<String>,
    repository_access: HashSet<(AccountId, RepositoryRef)>,
    host_create_access: HashSet<(AccountId, String)>,
    fail_repository_create: bool,
    fail_repository_delete: bool,
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

    fn host_for_repository_binding(
        &self,
        _repository: &RepositoryRef,
    ) -> Result<DesignatedHost, SourceControlError> {
        DesignatedHost::parse("source-control.local")
            .map_err(|_| SourceControlError::OperationFailed)
    }

    async fn ensure_provider_reachable(&self) -> Result<(), SourceControlError> {
        if self.config.provider_reachable {
            Ok(())
        } else {
            Err(SourceControlError::ProviderUnreachable)
        }
    }

    async fn ensure_host_reachable(&self, host: &DesignatedHost) -> Result<(), SourceControlError> {
        if self.config.reachable_hosts.contains(host.as_str()) {
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
        Ok(self
            .config
            .repository_access
            .contains(&(actor_account_id, repository.clone())))
    }

    async fn can_create_repository_at_host(
        &self,
        actor_account_id: AccountId,
        host: &DesignatedHost,
    ) -> Result<bool, SourceControlError> {
        Ok(self
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
    let mut config = EnvFixtureSourceControlConfig {
        provider_reachable: true,
        ..EnvFixtureSourceControlConfig::default()
    };
    for entry in raw[PREFIX.len()..].split(';') {
        if entry.is_empty() {
            continue;
        }
        let Some((key, value)) = entry.split_once('=') else {
            continue;
        };
        match key {
            "provider_reachable" => {
                config.provider_reachable = parse_bool_flag(value, true);
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
    Some(Arc::new(EnvFixtureSourceControlProvider {
        config,
        created_repositories: Arc::new(Mutex::new(HashSet::new())),
    }))
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
