//! Harness adapter contract.
//!
//! Codex, Claude Code, and `OpenCode` are required harness adapter families.
//! This crate defines only the trait surface and capability shape every
//! harness must satisfy. Concrete adapters (`tanren-harness-claude`,
//! `tanren-harness-codex`, `tanren-harness-opencode`, ...) are separate
//! crates introduced by the slice that first needs them, so additional
//! harness families can be added without disturbing F-0001 scope.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// What a harness can do. Reported by adapters at registration time so the
/// orchestration layer can route assignments to a compatible harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    /// Stable name of the capability (e.g. `"agentic-phase"`, `"gate"`).
    pub name: String,
    /// Free-form version string the adapter wants to expose. Compared as a
    /// string; semver semantics are the adapter's responsibility.
    pub version: String,
}

/// The harness trait every adapter implements. F-0001 ships only the
/// trait shape; impls live in separate adapter crates.
#[async_trait::async_trait]
pub trait Harness: Send + Sync {
    /// Report the capabilities this harness exposes.
    fn capabilities(&self) -> Vec<Capability>;

    /// Stable identifier for this harness family (`"codex"`, `"claude"`,
    /// `"opencode"`, ...).
    fn family(&self) -> &str;
}

/// Errors raised by harness adapters.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HarnessError {
    /// The harness binary failed to launch or returned a non-recoverable
    /// error.
    #[error("harness invocation failed: {0}")]
    Invocation(String),
}
