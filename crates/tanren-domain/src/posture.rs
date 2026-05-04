//! Deployment posture domain types.
//!
//! A posture represents the deployment-posture decision (hosted, self-hosted,
//! local-only) for a Tanren installation. The posture is scoped to the
//! **installation** — there is exactly one posture record per installation.
//! It is a top-level decision that gates first-run progress to providers
//! (R-0016) and ultimately to the ready-first-project milestone (R-0018).
//!
//! Neighbour crates:
//! - R-0016 owns provider selection (which depends on this posture choice).
//! - R-0017 owns the umbrella first-run checklist that surfaces this decision.
//! - M-0010 owns runtime placement that respects this posture.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

/// Deployment posture for a Tanren installation.
///
/// Scoped to the installation — there is exactly one posture record per
/// installation. Later runtime and credential choices inherit the selected
/// posture unless a user with permission changes it.
///
/// This is a closed enum: new variants require a domain-level decision and
/// migration; callers can match exhaustively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Posture {
    /// Tanren hosts the installation. The user interacts via the web / API /
    /// MCP surfaces; infrastructure and data residency are managed by Tanren.
    Hosted,
    /// The user hosts Tanren on their own infrastructure. Full control over
    /// data residency, networking, and runtime placement; the user is
    /// responsible for provisioning and maintenance.
    SelfHosted,
    /// The installation runs entirely on the user's local machine. No
    /// server-side infrastructure is required; capabilities that depend on
    /// shared infrastructure are unavailable.
    LocalOnly,
}

impl Posture {
    /// All supported postures, in canonical order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Hosted, Self::SelfHosted, Self::LocalOnly]
    }

    /// Parse a posture from a case-insensitive, whitespace-trimmed string.
    ///
    /// Accepts both `snake_case` and `kebab-case` forms.
    ///
    /// # Errors
    ///
    /// Returns [`PostureParseError`] when the input does not match any
    /// supported posture.
    pub fn parse(input: &str) -> Result<Self, PostureParseError> {
        let normalized = input.trim().to_lowercase().replace('-', "_");
        match normalized.as_str() {
            "hosted" => Ok(Self::Hosted),
            "self_hosted" => Ok(Self::SelfHosted),
            "local_only" => Ok(Self::LocalOnly),
            _ => Err(PostureParseError {
                input: input.to_owned(),
            }),
        }
    }
}

impl fmt::Display for Posture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Hosted => "hosted",
            Self::SelfHosted => "self_hosted",
            Self::LocalOnly => "local_only",
        };
        f.write_str(s)
    }
}

impl FromStr for Posture {
    type Err = PostureParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Error returned when a string does not match any supported [`Posture`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown posture: \"{input}\"")]
pub struct PostureParseError {
    /// The input that was rejected.
    pub input: String,
}

/// Coarse capability category.
///
/// Each category groups a family of related capabilities whose availability
/// depends on the chosen [`Posture`]. The taxonomy is intentionally coarse
/// (per spec risk #2): it lists categories and availability, not individual
/// runtime details (M-0010 owns those).
///
/// New categories may be added in future releases (`#[non_exhaustive]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CapabilityCategory {
    /// Runtime compute (container execution, job scheduling).
    Compute,
    /// Persistent data storage.
    Storage,
    /// External network connectivity and API access.
    Networking,
    /// Multi-user collaboration features.
    Collaboration,
    /// Credential and secret management.
    Secrets,
    /// External provider integration and connectivity.
    ProviderIntegration,
}

/// Availability of a capability within a given posture.
///
/// `Available` means the capability is fully usable. `Unavailable` carries a
/// human-readable reason explaining why the capability cannot be used (e.g.
/// "Local-only installations do not support multi-user collaboration").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CapabilityAvailability {
    /// The capability is available for use.
    Available,
    /// The capability is not available in the current posture.
    Unavailable {
        /// Human-readable explanation of why the capability is unavailable.
        reason: String,
    },
}

/// Summary of a capability's availability within a given posture.
///
/// Maps a [`CapabilityCategory`] to its [`CapabilityAvailability`]. A
/// collection of these (one per category) describes the full capability
/// surface for a posture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CapabilitySummary {
    /// The capability category this entry describes.
    pub category: CapabilityCategory,
    /// Whether the capability is available in the current posture.
    pub availability: CapabilityAvailability,
}
