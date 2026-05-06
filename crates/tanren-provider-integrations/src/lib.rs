//! Provider Integrations subsystem.
//!
//! Owns *outbound* connections to external systems: source control (GitHub,
//! GitLab), CI (GitHub Actions, Buildkite), issue trackers (Linear, Jira),
//! cloud or VM providers (Hetzner, GCP, AWS), and notification channels
//! (Slack, email). Concrete adapters live in separate crates introduced by
//! the slice that first needs each provider family.

use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::ProviderConnectionId;
use thiserror::Error;

/// Stable identifier for a provider family (`"github"`, `"local"`, ...).
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
#[async_trait]
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
    /// The requested resource was not found within the provider connection.
    #[error("resource not found: {0}")]
    ResourceNotFound(String),
}

/// Capabilities a source-control provider connection offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    /// Whether the connection can read repository metadata.
    pub can_read: bool,
    /// Whether the connection can assess merge permissions.
    pub can_assess_merge_permissions: bool,
}

/// Summary of merge permissions for a repository resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergePermissionSummary {
    /// Whether the authenticated user can push to the default branch.
    pub can_push_to_default: bool,
}

/// A resolved repository resource within a provider connection. Carries a
/// stable opaque identifier and a **redacted** display reference suitable
/// for storage and emission in API responses — never a secret-bearing URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryResource {
    /// Opaque identifier for the resource within the provider connection.
    pub resource_id: String,
    /// Redacted display reference (e.g. `github.com/acme/tanren-app`,
    /// `local://bdd-temp`). Never contains credentials.
    pub display_ref: String,
}

/// Source-control provider that can validate resources and check
/// capabilities. Adapters implement this trait; the handler resolves
/// resources and validates capabilities before connecting a project.
#[async_trait]
pub trait SourceControlProvider: Send + Sync + std::fmt::Debug {
    /// The provider family this adapter implements.
    fn family(&self) -> ProviderFamily;

    /// Capabilities this provider connection offers.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Resolve a repository resource by its opaque identifier within this
    /// provider connection.
    async fn resolve_resource(
        &self,
        resource_id: &str,
    ) -> Result<RepositoryResource, ProviderError>;

    /// Summarize merge permissions for a resolved repository resource.
    async fn merge_permissions(
        &self,
        resource: &RepositoryResource,
    ) -> Result<MergePermissionSummary, ProviderError>;
}

/// Registry of configured source-control provider connections. The handler
/// looks up a provider by connection ID to validate capabilities and
/// resolve resources before connecting a project.
#[async_trait]
pub trait ProviderRegistry: Send + Sync + std::fmt::Debug {
    /// Look up a source-control provider by its configured connection ID.
    async fn get(
        &self,
        connection_id: ProviderConnectionId,
    ) -> Option<Arc<dyn SourceControlProvider>>;
}

/// Void registry that always returns `None`. Used as the default in
/// `Handlers::new()` so that non-connect operations (health, sign-up,
/// sign-in, list, disconnect) continue to work without a provider
/// registry. Calling `connect_project` with this registry will fail
/// with `ProviderConnectionNotFound`.
#[derive(Debug, Clone)]
pub struct VoidProviderRegistry;

#[async_trait]
impl ProviderRegistry for VoidProviderRegistry {
    async fn get(
        &self,
        _connection_id: ProviderConnectionId,
    ) -> Option<Arc<dyn SourceControlProvider>> {
        None
    }
}

/// Deterministic local source-control provider fixture for testing and
/// BDD scenarios. Represents filesystem-based repository fixtures through
/// the typed provider contract without modifying repository bytes.
///
/// Every connection ID resolves to the same local provider. Resources are
/// identified by an opaque string and rendered as `local://<resource_id>`
/// in the redacted display reference.
#[derive(Debug)]
pub struct LocalSourceControlProvider {
    connection_id: ProviderConnectionId,
}

impl LocalSourceControlProvider {
    /// Create a new local provider with a fresh connection ID.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connection_id: ProviderConnectionId::fresh(),
        }
    }

    /// The connection ID this provider is registered under.
    #[must_use]
    pub fn connection_id(&self) -> ProviderConnectionId {
        self.connection_id
    }
}

impl Default for LocalSourceControlProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderAdapter for LocalSourceControlProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::new("local".to_owned())
    }
}

#[async_trait]
impl SourceControlProvider for LocalSourceControlProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::new("local".to_owned())
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            can_read: true,
            can_assess_merge_permissions: true,
        }
    }

    async fn resolve_resource(
        &self,
        resource_id: &str,
    ) -> Result<RepositoryResource, ProviderError> {
        let display_ref = format!("local://{resource_id}");
        Ok(RepositoryResource {
            resource_id: resource_id.to_owned(),
            display_ref,
        })
    }

    async fn merge_permissions(
        &self,
        _resource: &RepositoryResource,
    ) -> Result<MergePermissionSummary, ProviderError> {
        Ok(MergePermissionSummary {
            can_push_to_default: true,
        })
    }
}

/// Registry backed by a single local provider fixture. Returns the same
/// provider for **any** connection ID — this is a deterministic test fixture
/// that does not distinguish between real provider connections.
#[derive(Debug, Clone)]
pub struct LocalProviderRegistry {
    provider: Arc<LocalSourceControlProvider>,
}

impl LocalProviderRegistry {
    /// Create a registry wrapping a fresh local provider.
    #[must_use]
    pub fn new() -> Self {
        Self {
            provider: Arc::new(LocalSourceControlProvider::new()),
        }
    }

    /// The connection ID the wrapped provider was initialized with.
    #[must_use]
    pub fn connection_id(&self) -> ProviderConnectionId {
        self.provider.connection_id()
    }
}

impl Default for LocalProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProviderRegistry for LocalProviderRegistry {
    async fn get(
        &self,
        _connection_id: ProviderConnectionId,
    ) -> Option<Arc<dyn SourceControlProvider>> {
        Some(self.provider.clone())
    }
}
