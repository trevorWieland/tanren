//! Provider Integrations subsystem.
//!
//! Owns *outbound* connections to external systems: source control (GitHub,
//! GitLab), CI (GitHub Actions, Buildkite), issue trackers (Linear, Jira),
//! cloud or VM providers (Hetzner, GCP, AWS), and notification channels
//! (Slack, email). Concrete adapters live in separate crates introduced by
//! the slice that first needs each provider family.

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

/// The outbound provider trait every adapter implements.
#[async_trait::async_trait]
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
}
