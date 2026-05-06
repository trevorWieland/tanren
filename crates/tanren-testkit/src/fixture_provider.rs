#![cfg(feature = "test-hooks")]

//! In-memory source-control provider fixture for BDD scenarios.
//!
//! Models accessible hosts, inaccessible hosts, existing repositories,
//! and newly-created repositories. Checking access to an existing
//! repository does not mutate fixture state.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tanren_provider_integrations::{HostId, ProviderError, RepositoryInfo, SourceControlProvider};

#[derive(Debug)]
struct FixtureState {
    accessible_hosts: HashSet<HostId>,
    existing_repos: HashMap<String, RepositoryInfo>,
    created_repos: Vec<RepositoryInfo>,
}

/// In-memory source-control provider fixture.
///
/// Tracks accessible hosts, existing repositories, and newly-created
/// repositories. Use the builder methods to configure the fixture
/// before passing it to the code under test.
///
/// Hosts not explicitly registered via [`with_accessible_host`](Self::with_accessible_host)
/// are treated as inaccessible and produce
/// [`ProviderError::HostAccess`](tanren_provider_integrations::ProviderError::HostAccess)
/// on every call.
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
                accessible_hosts: HashSet::new(),
                existing_repos: HashMap::new(),
                created_repos: Vec::new(),
            })),
        }
    }

    /// Register a host as accessible (the caller has an active provider
    /// connection for it).
    #[must_use]
    pub fn with_accessible_host(self, host: &str) -> Self {
        let host_id = HostId::new(host.to_owned());
        self.inner
            .lock()
            .expect("fixture mutex poisoned")
            .accessible_hosts
            .insert(host_id);
        self
    }

    /// Register an existing repository. The repository is queryable via
    /// [`SourceControlProvider::check_repo_access`] when the matching host
    /// is also marked accessible.
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
        host: &HostId,
        url: &str,
    ) -> Result<RepositoryInfo, ProviderError> {
        let state = self.inner.lock().expect("fixture mutex poisoned");
        if !state.accessible_hosts.contains(host) {
            return Err(ProviderError::HostAccess(host.clone()));
        }
        match state.existing_repos.get(url) {
            Some(info) => Ok(info.clone()),
            None => Err(ProviderError::Call(format!("repository not found: {url}"))),
        }
    }

    async fn create_repository(
        &self,
        host: &HostId,
        name: &str,
    ) -> Result<RepositoryInfo, ProviderError> {
        let mut state = self.inner.lock().expect("fixture mutex poisoned");
        if !state.accessible_hosts.contains(host) {
            return Err(ProviderError::HostAccess(host.clone()));
        }
        let url = format!("https://{}/{name}", host.as_str());
        let info = RepositoryInfo { url };
        state.created_repos.push(info.clone());
        Ok(info)
    }
}
