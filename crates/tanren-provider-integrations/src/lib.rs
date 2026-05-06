//! Provider Integrations subsystem.
//!
//! Owns *outbound* connections to external systems: source control (GitHub,
//! GitLab), CI (GitHub Actions, Buildkite), issue trackers (Linear, Jira),
//! cloud or VM providers (Hetzner, GCP, AWS), and notification channels
//! (Slack, email). Concrete adapters live in separate crates introduced by
//! the slice that first needs each provider family.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::AccountId;
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

/// Typed provider action describing what the caller is asking the
/// source-control provider to do. Actions are constructed *before* the
/// external side effect is dispatched so that logging, auditing, and
/// policy checks can inspect the full intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProviderAction {
    /// Verify that the caller has access to the repository at `url`.
    CheckRepoAccess {
        /// Fully-qualified repository URL to verify.
        url: String,
    },
    /// Create a new repository named `name` on the SCM host.
    CreateRepository {
        /// Name for the new repository.
        name: String,
    },
}

/// Typed connection context passed to every [`SourceControlProvider`]
/// method. Carries the authenticated actor, the target SCM host, and
/// the requested provider action — giving the implementation enough
/// information to perform authorization checks before the external
/// side effect.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConnectionContext {
    /// Authenticated account issuing the request.
    pub actor: AccountId,
    /// Target SCM host for the operation.
    pub host: HostId,
    /// The specific action the caller is requesting.
    pub action: ProviderAction,
}

impl ProviderConnectionContext {
    /// Borrow the `url` when the action is [`ProviderAction::CheckRepoAccess`].
    #[must_use]
    pub fn check_repo_access_url(&self) -> Option<&str> {
        match &self.action {
            ProviderAction::CheckRepoAccess { url } => Some(url),
            _ => None,
        }
    }

    /// Borrow the `name` when the action is [`ProviderAction::CreateRepository`].
    #[must_use]
    pub fn create_repository_name(&self) -> Option<&str> {
        match &self.action {
            ProviderAction::CreateRepository { name } => Some(name),
            _ => None,
        }
    }
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
///
/// Every method receives a [`ProviderConnectionContext`] containing the
/// authenticated actor, target host, and the requested provider action.
/// Implementations use this context for authorization checks before
/// invoking the external side effect.
#[async_trait]
pub trait SourceControlProvider: Send + Sync {
    /// Verify that the caller identified by the connection context has
    /// access to the repository specified in the action. Returns
    /// [`RepositoryInfo`] on success.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::HostAccess`] when the caller has no
    /// active provider connection for the host, or [`ProviderError::Call`]
    /// when the provider rejects the request or the repository does not
    /// exist.
    async fn check_repo_access(
        &self,
        context: &ProviderConnectionContext,
    ) -> Result<RepositoryInfo, ProviderError>;

    /// Create a new repository on the designated host as specified in
    /// the connection context action. Returns [`RepositoryInfo`] for the
    /// newly created repository.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::HostAccess`] when the caller has no
    /// active provider connection for the host, or [`ProviderError::Call`]
    /// when the provider rejects the request or is unreachable.
    async fn create_repository(
        &self,
        context: &ProviderConnectionContext,
    ) -> Result<RepositoryInfo, ProviderError>;
}

/// Registry that resolves a [`SourceControlProvider`] at request time.
///
/// Production binaries wire a [`NullProviderRegistry`] when no SCM backend
/// is available, causing every project command to fail fast with
/// [`ProviderError::NotConfigured`]. Future slices that introduce real
/// adapters (GitHub, GitLab, etc.) will swap in a registry that looks up
/// the correct provider by host.
///
/// BDD harnesses inject a [`FixedProviderRegistry`] wrapping a
/// [`FixtureSourceControlProvider`](crate::FixedProviderProvider) so that
/// scenarios can observe both the happy path and the not-configured
/// failure path.
pub trait ProviderRegistry: Send + Sync {
    /// Resolve the source-control provider for the current request.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::NotConfigured`] when no provider is
    /// available, or [`ProviderError::HostAccess`] when the requested
    /// host has no active provider connection.
    fn resolve(&self) -> Result<Arc<dyn SourceControlProvider>, ProviderError>;
}

/// Registry that always returns [`ProviderError::NotConfigured`].
///
/// Used as the default registry in production builds until a real SCM
/// adapter is wired in by a future slice.
pub struct NullProviderRegistry;

impl fmt::Debug for NullProviderRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NullProviderRegistry").finish()
    }
}

impl ProviderRegistry for NullProviderRegistry {
    fn resolve(&self) -> Result<Arc<dyn SourceControlProvider>, ProviderError> {
        Err(ProviderError::NotConfigured)
    }
}

/// Registry wrapping a single, optional [`SourceControlProvider`].
///
/// Returns the inner provider when present, or
/// [`ProviderError::NotConfigured`] when absent. Test-hook constructors
/// build this from an `Option<Arc<dyn SourceControlProvider>>` so that
/// existing BDD harness code does not need to change.
pub struct FixedProviderRegistry {
    provider: Option<Arc<dyn SourceControlProvider>>,
}

impl fmt::Debug for FixedProviderRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FixedProviderRegistry")
            .field(
                "provider",
                &self
                    .provider
                    .as_ref()
                    .map(|_| "Some(<SourceControlProvider>)"),
            )
            .finish()
    }
}

impl FixedProviderRegistry {
    /// Create a registry from an optional provider.
    #[must_use]
    pub fn new(provider: Option<Arc<dyn SourceControlProvider>>) -> Self {
        Self { provider }
    }
}

impl ProviderRegistry for FixedProviderRegistry {
    fn resolve(&self) -> Result<Arc<dyn SourceControlProvider>, ProviderError> {
        self.provider.clone().ok_or(ProviderError::NotConfigured)
    }
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
    /// No SCM provider is configured for the deployment.
    #[error("no SCM provider configured")]
    NotConfigured,
}
