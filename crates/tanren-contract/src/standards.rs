//! Standards inspection wire shapes.
//!
//! Request, response, and failure types for inspecting the standards
//! installed in a Tanren repository. Every interface (api, mcp, cli,
//! tui, web) serialises these same shapes — keeping them in
//! `tanren-contract` is the architectural guarantee that all surfaces
//! stay equivalent.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

const STANDARD_SCHEMA_PREFIX: &str = "tanren.standards.";

// ── Closed enum types ────────────────────────────────────────────────

/// Kind of a standard within the Tanren standards taxonomy.
///
/// Distinguishes the structural role of a standards file.
/// [`#[non_exhaustive]`](non_exhaustive) accommodates future kinds
/// without wire-breaking changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StandardKind {
    /// A codified standard enforced by tooling or review.
    Standard,
    /// A mandatory policy that must be satisfied.
    Policy,
    /// A recommended guideline that is not enforced.
    Guideline,
    /// An agreed-upon convention adopted by the team.
    Convention,
}

/// Category grouping for a standard.
///
/// Groups standards by their subject area. [`#[non_exhaustive]`](non_exhaustive)
/// allows additional categories as the standards taxonomy evolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StandardCategory {
    /// Code quality, formatting, and linting standards.
    CodeQuality,
    /// Testing strategy, coverage, and convention standards.
    Testing,
    /// Documentation structure and content standards.
    Documentation,
    /// Security policy and practice standards.
    Security,
    /// Architecture decision and boundary standards.
    Architecture,
    /// Development process and workflow standards.
    Process,
}

/// Importance level of a standard within a project.
///
/// Indicates how strictly a standard should be followed.
/// [`#[non_exhaustive]`](non_exhaustive) for forward-compatible growth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StandardImportance {
    /// The standard must be followed; violations block merges or releases.
    Required,
    /// The standard should be followed; deviations are flagged but not blocking.
    Recommended,
    /// The standard is provided as context; no enforcement is expected.
    Informational,
}

/// Projection status of a standard within a repository.
///
/// Describes whether a standard is currently active, deprecated, or
/// pending activation. [`#[non_exhaustive]`](non_exhaustive) for
/// forward-compatible growth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StandardStatus {
    /// The standard is active and enforced.
    Active,
    /// The standard is deprecated and should not be used in new work.
    Deprecated,
    /// The standard is pending review or activation.
    Pending,
}

// ── StandardSchema newtype ───────────────────────────────────────────

/// Schema version of the standards configuration.
///
/// Wraps the raw schema identifier (e.g. `"tanren.standards.v0"`) so
/// callers cannot pass an arbitrary unvalidated string where a schema
/// version is expected. Construct via [`StandardSchema::parse`] or
/// [`StandardSchema::current`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema, ToSchema)]
#[serde(transparent)]
pub struct StandardSchema(String);

impl StandardSchema {
    /// The current canonical standards schema version string.
    pub const V0: &str = "tanren.standards.v0";

    /// Construct the current canonical schema version.
    #[must_use]
    pub fn current() -> Self {
        Self(Self::V0.to_owned())
    }

    /// Parse a raw schema string into a validated [`StandardSchema`].
    ///
    /// # Errors
    ///
    /// Returns [`StandardsSchemaError::Empty`] if the input is empty
    /// after trimming, or [`StandardsSchemaError::InvalidPrefix`] if
    /// the input does not start with `"tanren.standards."`.
    pub fn parse(raw: &str) -> Result<Self, StandardsSchemaError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(StandardsSchemaError::Empty);
        }
        if !trimmed.starts_with(STANDARD_SCHEMA_PREFIX) {
            return Err(StandardsSchemaError::InvalidPrefix {
                input: trimmed.to_owned(),
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the underlying schema identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for StandardSchema {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for StandardSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validation errors for [`StandardSchema`] construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum StandardsSchemaError {
    /// The supplied schema string was empty after trimming.
    #[error("standards schema is empty")]
    Empty,
    /// The supplied schema string does not start with the required prefix.
    #[error("standards schema must start with '{STANDARD_SCHEMA_PREFIX}', got: {input}")]
    InvalidPrefix { input: String },
}

// ── Request / response shapes ────────────────────────────────────────

/// Request to inspect the standards installed in a repository.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct StandardsInspectionRequest {
    /// Absolute or relative path to the repository root.
    pub project_dir: String,
}

/// Response from a successful standards inspection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct StandardsInspectionResponse {
    /// The resolved standards root directory.
    pub standards_root: String,
    /// The standards discovered in the configured root.
    pub standards: Vec<StandardView>,
}

impl StandardsInspectionResponse {
    /// Number of standards in this response.
    #[must_use]
    pub fn count(&self) -> usize {
        self.standards.len()
    }
}

/// View of a single standard within a [`StandardsInspectionResponse`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct StandardView {
    /// Schema version of this standard's format.
    pub schema: StandardSchema,
    /// Structural kind of the standard.
    pub kind: StandardKind,
    /// Subject-area category.
    pub category: StandardCategory,
    /// Importance level for the project.
    pub importance: StandardImportance,
    /// Current projection status.
    pub status: StandardStatus,
    /// Human-readable name of the standard.
    pub name: String,
    /// Relative path within the standards root.
    pub path: String,
}

// ── Failure taxonomy ─────────────────────────────────────────────────

/// Closed taxonomy of standards-inspection failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects a
/// [`StandardsFailureReason`] into the same wire shape so callers can
/// match on `code` regardless of transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum StandardsFailureReason {
    /// The configured standards root directory does not exist.
    StandardsRootNotFound,
    /// A standards file could not be parsed.
    StandardsFileMalformed,
    /// The configured root exists but contains no valid standards.
    StandardsEmpty,
    /// The standards configuration uses an unsupported schema version.
    InvalidSchema,
    /// A path violated security constraints (absolute, escaping, or symlink).
    PathViolation,
    /// The standards tree exceeded structural bounds (size, count, or depth).
    TreeBoundsExceeded,
}

impl StandardsFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::StandardsRootNotFound => "standards_root_not_found",
            Self::StandardsFileMalformed => "standards_file_malformed",
            Self::StandardsEmpty => "standards_empty",
            Self::InvalidSchema => "invalid_schema",
            Self::PathViolation => "path_violation",
            Self::TreeBoundsExceeded => "tree_bounds_exceeded",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::StandardsRootNotFound => {
                "The configured standards root directory does not exist."
            }
            Self::StandardsFileMalformed => "A standards file could not be parsed.",
            Self::StandardsEmpty => "The configured root exists but contains no valid standards.",
            Self::InvalidSchema => {
                "The standards configuration uses an unsupported schema version."
            }
            Self::PathViolation => {
                "A path violated security constraints (absolute, escaping, or symlink)."
            }
            Self::TreeBoundsExceeded => "The standards tree exceeded structural bounds.",
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces. Centralized so every transport reports the
    /// same status for the same failure code.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::StandardsRootNotFound | Self::StandardsEmpty => 404,
            Self::StandardsFileMalformed | Self::TreeBoundsExceeded => 422,
            Self::InvalidSchema | Self::PathViolation => 400,
        }
    }
}
