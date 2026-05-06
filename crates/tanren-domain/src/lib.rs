//! Canonical Tanren domain entities.
//!
//! This crate is the foundation of the workspace dependency layering: it has
//! no workspace dependencies. Other crates may depend on `tanren-domain`;
//! `tanren-domain` depends on nothing else from the workspace.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Schema version for the canonical domain model.
///
/// Bumped on breaking shape changes to the domain types this crate exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DomainVersion(u32);

impl DomainVersion {
    /// Current domain schema version.
    pub const CURRENT: Self = Self(0);

    /// Construct a domain version from its numeric form.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// The numeric value of this schema version.
    #[must_use]
    pub const fn value(self) -> u32 {
        self.0
    }
}

/// Canonical deployment posture for a Tanren installation or account scope.
///
/// The posture determines which capabilities are available. It is a top-level
/// decision that gates first-run progress to provider selection and runtime
/// configuration.
///
/// The contract crate (`tanren-contract`) defines a parallel wire-shape type
/// with schema derives; this is the canonical domain definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentPosture {
    /// Tanren operates as a managed hosted service.
    Hosted,
    /// Tanren operates as self-hosted infrastructure.
    SelfHosted,
    /// Tanren operates in a local-only mode with reduced capabilities.
    LocalOnly,
}

/// Errors raised when a domain-level invariant is violated.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DomainError {
    /// A domain invariant was violated. The argument names which invariant.
    #[error("domain invariant violated: {0}")]
    InvariantViolation(String),
}
