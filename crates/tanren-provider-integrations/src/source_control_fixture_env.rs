use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use tanren_identity_policy::{AccountId, DesignatedHost, ProviderFamily, RepositoryRef};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    SourceControlConnectPreflight, SourceControlCreatePreflight, SourceControlError,
    SourceControlProvider, SourceControlRateLimitStatus, SourceControlReachabilityStatus,
    SourceControlRemoteIdentity,
};

/// Errors returned when parsing a fixture environment configuration string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum FixtureEnvParseError {
    /// The input does not start with the required `fixture-v1;` prefix.
    #[error("fixture env: missing required prefix 'fixture-v1;'")]
    MissingPrefix,
    /// An entry is missing the `key=value` separator.
    #[error("fixture env: entry {entry_index} is missing '=' separator")]
    MissingSeparator {
        /// Zero-based index of the malformed entry within the semicolon-delimited list.
        entry_index: usize,
    },
    /// An entry uses a key not recognised by the fixture parser.
    #[error("fixture env: unknown key '{key}' at entry {entry_index}")]
    UnknownKey {
        /// The unrecognised key.
        key: String,
        /// Zero-based entry index.
        entry_index: usize,
    },
    /// A boolean flag value is not one of the accepted literals.
    #[error("fixture env: invalid boolean '{value}' for key '{key}' at entry {entry_index}")]
    InvalidBoolean {
        /// The key that expected a boolean value.
        key: String,
        /// The raw value that could not be parsed.
        value: String,
        /// Zero-based entry index.
        entry_index: usize,
    },
    /// A host value in `reachable_hosts` is empty after trimming.
    #[error(
        "fixture env: empty host token in reachable_hosts at entry {entry_index}, token {token_index}"
    )]
    EmptyHost {
        /// Zero-based entry index.
        entry_index: usize,
        /// Zero-based token index within the pipe-delimited value.
        token_index: usize,
    },
    /// A repository-access pair could not be parsed.
    #[error(
        "fixture env: malformed repo_access entry '{raw}' at entry {entry_index}, token {token_index}"
    )]
    MalformedRepoAccess {
        /// The raw token that failed to parse.
        raw: String,
        /// Zero-based entry index.
        entry_index: usize,
        /// Zero-based token index within the pipe-delimited value.
        token_index: usize,
    },
    /// An account id within a repository-access pair is not a valid `UUIDv7`.
    #[error(
        "fixture env: invalid account id '{raw}' in repo_access at entry {entry_index}, token {token_index}"
    )]
    InvalidRepoAccessAccountId {
        /// The raw account-id token.
        raw: String,
        /// Zero-based entry index.
        entry_index: usize,
        /// Zero-based token index.
        token_index: usize,
    },
    /// A repository ref within a repository-access pair is not canonical.
    #[error(
        "fixture env: invalid repository ref '{raw}' in repo_access at entry {entry_index}, token {token_index}"
    )]
    InvalidRepoAccessRepositoryRef {
        /// The raw repository-ref token.
        raw: String,
        /// Zero-based entry index.
        entry_index: usize,
        /// Zero-based token index.
        token_index: usize,
    },
    /// A host-create-access pair could not be parsed.
    #[error(
        "fixture env: malformed host_create_access entry '{raw}' at entry {entry_index}, token {token_index}"
    )]
    MalformedHostCreateAccess {
        /// The raw token that failed to parse.
        raw: String,
        /// Zero-based entry index.
        entry_index: usize,
        /// Zero-based token index within the pipe-delimited value.
        token_index: usize,
    },
    /// An account id within a host-create-access pair is not a valid `UUIDv7`.
    #[error(
        "fixture env: invalid account id '{raw}' in host_create_access at entry {entry_index}, token {token_index}"
    )]
    InvalidHostCreateAccessAccountId {
        /// The raw account-id token.
        raw: String,
        /// Zero-based entry index.
        entry_index: usize,
        /// Zero-based token index.
        token_index: usize,
    },
    /// A host within a host-create-access pair is empty after trimming.
    #[error(
        "fixture env: empty host in host_create_access at entry {entry_index}, token {token_index}"
    )]
    EmptyHostCreateAccessHost {
        /// Zero-based entry index.
        entry_index: usize,
        /// Zero-based token index.
        token_index: usize,
    },
}

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

/// Parse a `fixture-v1;...` environment value into a typed provider.
///
/// Returns a typed error for every class of malformed input instead of
/// silently falling back to defaults.
///
/// # Errors
///
/// Returns [`FixtureEnvParseError`] variants for:
/// - missing `fixture-v1;` prefix,
/// - entries without the `key=value` separator,
/// - unknown keys,
/// - boolean values that are not one of `1`, `true`, `TRUE`, `True`, `0`,
///   `false`, `FALSE`, `False`,
/// - empty host tokens in `reachable_hosts`,
/// - malformed `account@repo` pairs in `repo_access`,
/// - malformed `account@host` pairs in `host_create_access`.
pub(crate) fn env_fixture_source_control_provider(
    raw: &str,
) -> Result<Arc<dyn SourceControlProvider>, FixtureEnvParseError> {
    const PREFIX: &str = "fixture-v1;";
    if !raw.starts_with(PREFIX) {
        return Err(FixtureEnvParseError::MissingPrefix);
    }
    let mut config = EnvFixtureSourceControlConfig::default();
    let mut provider_reachable = true;
    let mut rate_limited = false;
    for (entry_index, entry) in raw[PREFIX.len()..].split(';').enumerate() {
        if entry.is_empty() {
            continue;
        }
        let Some((key, value)) = entry.split_once('=') else {
            return Err(FixtureEnvParseError::MissingSeparator { entry_index });
        };
        apply_entry(
            key,
            value,
            entry_index,
            &mut config,
            &mut provider_reachable,
            &mut rate_limited,
        )?;
    }
    config.preflight_mode = resolve_preflight_mode(provider_reachable, rate_limited);
    Ok(Arc::new(EnvFixtureSourceControlProvider {
        config,
        created_repositories: Arc::new(Mutex::new(HashSet::new())),
    }))
}

fn resolve_preflight_mode(provider_reachable: bool, rate_limited: bool) -> EnvFixturePreflightMode {
    if !provider_reachable {
        EnvFixturePreflightMode::ProviderUnreachable
    } else if rate_limited {
        EnvFixturePreflightMode::RateLimited
    } else {
        EnvFixturePreflightMode::Ready
    }
}

fn apply_entry(
    key: &str,
    value: &str,
    entry_index: usize,
    config: &mut EnvFixtureSourceControlConfig,
    provider_reachable: &mut bool,
    rate_limited: &mut bool,
) -> Result<(), FixtureEnvParseError> {
    match key {
        "provider_reachable" => {
            *provider_reachable =
                parse_bool_flag(value).map_err(|value| FixtureEnvParseError::InvalidBoolean {
                    key: key.to_owned(),
                    value,
                    entry_index,
                })?;
        }
        "rate_limited" => {
            *rate_limited =
                parse_bool_flag(value).map_err(|value| FixtureEnvParseError::InvalidBoolean {
                    key: key.to_owned(),
                    value,
                    entry_index,
                })?;
        }
        "fail_repository_create" => {
            config.fail_repository_create =
                parse_bool_flag(value).map_err(|value| FixtureEnvParseError::InvalidBoolean {
                    key: key.to_owned(),
                    value,
                    entry_index,
                })?;
        }
        "fail_repository_delete" => {
            config.fail_repository_delete =
                parse_bool_flag(value).map_err(|value| FixtureEnvParseError::InvalidBoolean {
                    key: key.to_owned(),
                    value,
                    entry_index,
                })?;
        }
        "reachable_hosts" => {
            for (token_index, host) in value
                .split('|')
                .filter(|token| !token.trim().is_empty())
                .enumerate()
            {
                let trimmed = host.trim().to_ascii_lowercase();
                if trimmed.is_empty() {
                    return Err(FixtureEnvParseError::EmptyHost {
                        entry_index,
                        token_index,
                    });
                }
                config.reachable_hosts.insert(trimmed);
            }
        }
        "repo_access" => {
            for (token_index, raw_pair) in value
                .split('|')
                .filter(|token| !token.trim().is_empty())
                .enumerate()
            {
                let pair = parse_repo_access_pair(raw_pair.trim(), entry_index, token_index)?;
                config.repository_access.insert(pair);
            }
        }
        "host_create_access" => {
            for (token_index, raw_pair) in value
                .split('|')
                .filter(|token| !token.trim().is_empty())
                .enumerate()
            {
                let pair = parse_host_access_pair(raw_pair.trim(), entry_index, token_index)?;
                config.host_create_access.insert(pair);
            }
        }
        _ => {
            return Err(FixtureEnvParseError::UnknownKey {
                key: key.to_owned(),
                entry_index,
            });
        }
    }
    Ok(())
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

fn parse_bool_flag(raw: &str) -> Result<bool, String> {
    match raw.trim() {
        "1" | "true" | "TRUE" | "True" => Ok(true),
        "0" | "false" | "FALSE" | "False" => Ok(false),
        other => Err(other.to_owned()),
    }
}

fn parse_repo_access_pair(
    raw: &str,
    entry_index: usize,
    token_index: usize,
) -> Result<(AccountId, RepositoryRef), FixtureEnvParseError> {
    let Some((account_raw, repository_raw)) = raw.split_once('@') else {
        return Err(FixtureEnvParseError::MalformedRepoAccess {
            raw: raw.to_owned(),
            entry_index,
            token_index,
        });
    };
    let account =
        parse_account_id(account_raw.trim(), entry_index, token_index).map_err(|raw| {
            FixtureEnvParseError::InvalidRepoAccessAccountId {
                raw,
                entry_index,
                token_index,
            }
        })?;
    let repository = RepositoryRef::parse(repository_raw.trim()).map_err(|_| {
        FixtureEnvParseError::InvalidRepoAccessRepositoryRef {
            raw: repository_raw.trim().to_owned(),
            entry_index,
            token_index,
        }
    })?;
    Ok((account, repository))
}

fn parse_host_access_pair(
    raw: &str,
    entry_index: usize,
    token_index: usize,
) -> Result<(AccountId, String), FixtureEnvParseError> {
    let Some((account_raw, host_raw)) = raw.split_once('@') else {
        return Err(FixtureEnvParseError::MalformedHostCreateAccess {
            raw: raw.to_owned(),
            entry_index,
            token_index,
        });
    };
    let account =
        parse_account_id(account_raw.trim(), entry_index, token_index).map_err(|raw| {
            FixtureEnvParseError::InvalidHostCreateAccessAccountId {
                raw,
                entry_index,
                token_index,
            }
        })?;
    let host = host_raw.trim().to_ascii_lowercase();
    if host.is_empty() {
        return Err(FixtureEnvParseError::EmptyHostCreateAccessHost {
            entry_index,
            token_index,
        });
    }
    Ok((account, host))
}

fn parse_account_id(
    raw: &str,
    _entry_index: usize,
    _token_index: usize,
) -> Result<AccountId, String> {
    let uuid = Uuid::parse_str(raw).map_err(|_| raw.to_owned())?;
    AccountId::try_from_uuid(uuid).map_err(|_| raw.to_owned())
}
