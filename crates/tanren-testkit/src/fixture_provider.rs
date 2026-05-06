#![cfg(feature = "test-hooks")]

//! In-memory source-control provider fixture for BDD scenarios.
//!
//! Models accessible hosts, actor-host connections, existing repositories,
//! and newly-created repositories. Checking access to an existing
//! repository does not mutate fixture state.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tanren_identity_policy::AccountId;
use tanren_provider_integrations::{
    HostId, ProviderConnectionContext, ProviderError, RepositoryInfo, SourceControlProvider,
};

#[derive(Debug)]
struct FixtureState {
    not_configured: bool,
    globally_accessible_hosts: HashSet<HostId>,
    actor_connections: HashSet<(AccountId, HostId)>,
    existing_repos: HashMap<String, RepositoryInfo>,
    created_repos: Vec<RepositoryInfo>,
}

impl FixtureState {
    fn is_accessible(&self, actor: &AccountId, host: &HostId) -> bool {
        self.globally_accessible_hosts.contains(host)
            || self.actor_connections.contains(&(*actor, host.clone()))
    }
}

/// In-memory source-control provider fixture.
///
/// Tracks accessible hosts, actor-host connections, existing repositories,
/// and newly-created repositories. Use the builder methods to configure
/// the fixture before passing it to the code under test.
///
/// Hosts not explicitly registered via [`with_accessible_host`](Self::with_accessible_host)
/// or [`with_actor_connection`](Self::with_actor_connection) are treated as
/// inaccessible and produce
/// [`ProviderError::HostAccess`](tanren_provider_integrations::ProviderError::HostAccess)
/// on every call.
///
/// # Access model
///
/// Access is determined by the connection context passed to each
/// [`SourceControlProvider`] method:
///
/// - **Global host access**: hosts registered via
///   [`with_accessible_host`](Self::with_accessible_host) allow any actor.
/// - **Per-actor access**: specific `(AccountId, HostId)` pairs registered
///   via [`with_actor_connection`](Self::with_actor_connection) allow only
///   the named actor on that host.
///
/// Both checks are evaluated, so a globally accessible host grants access
/// regardless of actor.
///
/// # Immutability of existing repositories
///
/// [`SourceControlProvider::check_repo_access`] only reads from the
/// pre-registered repository set — calling it never adds, removes, or
/// mutates repository entries.
#[derive(Debug, Clone)]
pub struct FixtureSourceControlProvider {
    inner: Arc<Mutex<FixtureState>>,
}

impl FixtureSourceControlProvider {
    /// Create an empty fixture with no hosts or repositories.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FixtureState {
                not_configured: false,
                globally_accessible_hosts: HashSet::new(),
                actor_connections: HashSet::new(),
                existing_repos: HashMap::new(),
                created_repos: Vec::new(),
            })),
        }
    }

    /// Register a host as globally accessible (any authenticated actor
    /// has an active provider connection for it).
    #[must_use]
    pub fn with_accessible_host(self, host: &str) -> Self {
        let host_id = HostId::new(host.to_owned());
        self.inner
            .lock()
            .expect("fixture mutex poisoned")
            .globally_accessible_hosts
            .insert(host_id);
        self
    }

    /// Register a specific actor as having an active provider connection
    /// for the given host. This grants access only to that actor on that
    /// host.
    #[must_use]
    pub fn with_actor_connection(self, actor: AccountId, host: &str) -> Self {
        let host_id = HostId::new(host.to_owned());
        self.inner
            .lock()
            .expect("fixture mutex poisoned")
            .actor_connections
            .insert((actor, host_id));
        self
    }

    /// Register an existing repository. The repository is queryable via
    /// [`SourceControlProvider::check_repo_access`] when the matching host
    /// is also accessible for the requesting actor.
    #[must_use]
    pub fn with_existing_repository(self, url: &str) -> Self {
        let info = RepositoryInfo {
            url: url.to_owned(),
        };
        self.inner
            .lock()
            .expect("fixture mutex poisoned")
            .existing_repos
            .insert(url.to_owned(), info);
        self
    }

    /// Mark the provider as not configured. All subsequent calls to
    /// [`SourceControlProvider`] methods return
    /// [`ProviderError::NotConfigured`](tanren_provider_integrations::ProviderError::NotConfigured).
    #[must_use]
    pub fn with_not_configured(self) -> Self {
        self.inner
            .lock()
            .expect("fixture mutex poisoned")
            .not_configured = true;
        self
    }

    /// Return the list of repositories created via
    /// [`SourceControlProvider::create_repository`].
    pub fn created_repositories(&self) -> Vec<RepositoryInfo> {
        self.inner
            .lock()
            .expect("fixture mutex poisoned")
            .created_repos
            .clone()
    }
}

impl Default for FixtureSourceControlProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SourceControlProvider for FixtureSourceControlProvider {
    async fn check_repo_access(
        &self,
        context: &ProviderConnectionContext,
    ) -> Result<RepositoryInfo, ProviderError> {
        let state = self.inner.lock().expect("fixture mutex poisoned");
        if state.not_configured {
            return Err(ProviderError::NotConfigured);
        }
        if !state.is_accessible(&context.actor, &context.host) {
            return Err(ProviderError::HostAccess(context.host.clone()));
        }
        let Some(url) = context.check_repo_access_url() else {
            return Err(ProviderError::Call(
                "unexpected action type for check_repo_access".to_owned(),
            ));
        };
        match state.existing_repos.get(url) {
            Some(info) => Ok(info.clone()),
            None => Err(ProviderError::Call(format!("repository not found: {url}"))),
        }
    }

    async fn create_repository(
        &self,
        context: &ProviderConnectionContext,
    ) -> Result<RepositoryInfo, ProviderError> {
        let mut state = self.inner.lock().expect("fixture mutex poisoned");
        if state.not_configured {
            return Err(ProviderError::NotConfigured);
        }
        if !state.is_accessible(&context.actor, &context.host) {
            return Err(ProviderError::HostAccess(context.host.clone()));
        }
        let Some(name) = context.create_repository_name() else {
            return Err(ProviderError::Call(
                "unexpected action type for create_repository".to_owned(),
            ));
        };
        let url = format!("https://{}/{name}", context.host.as_str());
        let info = RepositoryInfo { url };
        state.created_repos.push(info.clone());
        Ok(info)
    }
}
