//! Provider Integrations subsystem.
//!
//! Owns *outbound* connections to external systems: source control (GitHub,
//! GitLab), CI (GitHub Actions, Buildkite), issue trackers (Linear, Jira),
//! cloud or VM providers (Hetzner, GCP, AWS), and notification channels
//! (Slack, email). Concrete adapters live in separate crates introduced by
//! the slice that first needs each provider family.

use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

/// Identifier for an SCM host (e.g., `"github.com"`, `"gitlab.com"`).
///
/// Used by [`SourceControlProvider`] to route operations to the correct
/// upstream host. Hosts that the caller has no active provider connection
/// for produce a [`ProviderError::HostAccess`] failure.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HostId(String);

impl HostId {
    /// Wrap a host identifier.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the host identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Metadata about a repository on an SCM host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryInfo {
    /// Fully-qualified URL of the repository on the SCM host.
    pub url: String,
}

/// The outbound provider trait every adapter implements.
#[async_trait]
pub trait ProviderAdapter: Send + Sync {
    /// Family this adapter implements.
    fn family(&self) -> ProviderFamily;
}

/// Source-control provider trait for checking repository access and
/// creating new repositories at a designated SCM host.
///
/// Concrete implementations (GitHub, GitLab, etc.) are owned by M-0009.
/// This trait supplies the seam that project-registration flows
/// (B-0025, B-0026) call into without knowing the upstream provider.
#[async_trait]
pub trait SourceControlProvider: Send + Sync {
    /// Verify that the caller has access to the repository at `url` on
    /// the given `host`. Returns [`RepositoryInfo`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::HostAccess`] when the caller has no
    /// active provider connection for `host`, or [`ProviderError::Call`]
    /// when the provider rejects the request or the repository does not
    /// exist.
    async fn check_repo_access(
        &self,
        host: &HostId,
        url: &str,
    ) -> Result<RepositoryInfo, ProviderError>;

    /// Create a new repository named `name` on the designated `host`.
    /// Returns [`RepositoryInfo`] for the newly created repository.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::HostAccess`] when the caller has no
    /// active provider connection for `host`, or [`ProviderError::Call`]
    /// when the provider rejects the request or is unreachable.
    async fn create_repository(
        &self,
        host: &HostId,
        name: &str,
    ) -> Result<RepositoryInfo, ProviderError>;
}

/// Errors raised by provider integrations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProviderError {
    /// The caller does not have an active provider connection for the
    /// requested host.
    #[error("no provider connection for host: {0}")]
    HostAccess(HostId),
    /// The provider call could not complete.
    #[error("provider call failed: {0}")]
    Call(String),
}
