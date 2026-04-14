//! Inbound request types for contract operations.
//!
//! These types represent the canonical input shapes consumed by all
//! transport interfaces (CLI, API, MCP, TUI). Validation happens in
//! the [`TryFrom`] conversion to domain commands.

use std::collections::{HashMap, hash_map::Entry};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::enums::{AuthMode, Cli, DispatchMode, DispatchStatus, Lane, Phase};
use crate::error::ContractError;

/// Request to create a new dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateDispatchRequest {
    // Actor identity is sourced from trusted request context, not request payload.
    // -- Required dispatch fields --
    pub project: String,
    /// Phase of work.
    pub phase: Phase,
    /// CLI harness.
    pub cli: Cli,
    pub branch: String,
    pub spec_folder: String,
    pub workflow_id: String,
    /// Dispatch mode.
    pub mode: DispatchMode,
    /// Timeout in seconds (must be > 0).
    pub timeout_secs: u64,
    pub environment_profile: String,

    // -- Optional dispatch fields --
    /// Authentication mode.
    #[serde(default)]
    pub auth_mode: AuthMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Non-secret environment variables for the dispatch.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub project_env: HashMap<String, String>,
    /// Secret names required at runtime (not values).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_secrets: Vec<String>,
    /// Whether to preserve the environment on failure.
    #[serde(default)]
    pub preserve_on_failure: bool,
}

/// Filter parameters for listing dispatches.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchListFilter {
    /// Filter by dispatch status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<DispatchStatus>,
    /// Filter by concurrency lane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lane: Option<Lane>,
    /// Filter by project name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Maximum number of results to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    /// Opaque, versioned cursor token returned by a previous list response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<DispatchCursorToken>,
}

/// Typed, versioned dispatch list cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchCursorToken {
    /// Encoding version.
    pub version: u8,
    /// Primary sort key.
    pub created_at: DateTime<Utc>,
    /// Tie-break sort key.
    pub dispatch_id: Uuid,
}

impl DispatchCursorToken {
    pub const VERSION: u8 = 1;

    #[must_use]
    pub fn new(created_at: DateTime<Utc>, dispatch_id: Uuid) -> Self {
        Self {
            version: Self::VERSION,
            created_at,
            dispatch_id,
        }
    }

    #[must_use]
    pub fn encode(self) -> String {
        format!(
            "v{}|{}|{}",
            self.version,
            self.created_at.to_rfc3339(),
            self.dispatch_id
        )
    }

    pub fn decode(raw: &str) -> Result<Self, ContractError> {
        let (version_raw, rest) =
            raw.split_once('|')
                .ok_or_else(|| ContractError::InvalidField {
                    field: "cursor".to_owned(),
                    reason: "invalid cursor format".to_owned(),
                })?;
        let version = version_raw
            .strip_prefix('v')
            .ok_or_else(|| ContractError::InvalidField {
                field: "cursor".to_owned(),
                reason: "missing cursor version prefix".to_owned(),
            })?
            .parse::<u8>()
            .map_err(|_| ContractError::InvalidField {
                field: "cursor".to_owned(),
                reason: "invalid cursor version".to_owned(),
            })?;

        if version != Self::VERSION {
            return Err(ContractError::InvalidField {
                field: "cursor".to_owned(),
                reason: format!("unsupported cursor version {version}"),
            });
        }

        let (created_at_raw, dispatch_id_raw) =
            rest.split_once('|')
                .ok_or_else(|| ContractError::InvalidField {
                    field: "cursor".to_owned(),
                    reason: "invalid cursor payload format".to_owned(),
                })?;
        let created_at = DateTime::parse_from_rfc3339(created_at_raw).map_err(|_| {
            ContractError::InvalidField {
                field: "cursor".to_owned(),
                reason: "invalid cursor timestamp".to_owned(),
            }
        })?;
        let dispatch_id =
            Uuid::parse_str(dispatch_id_raw).map_err(|_| ContractError::InvalidField {
                field: "cursor".to_owned(),
                reason: "invalid cursor dispatch_id".to_owned(),
            })?;

        Ok(Self::new(created_at.with_timezone(&Utc), dispatch_id))
    }
}

impl Serialize for DispatchCursorToken {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.encode())
    }
}

impl<'de> Deserialize<'de> for DispatchCursorToken {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::decode(&raw).map_err(serde::de::Error::custom)
    }
}

/// Request to cancel a dispatch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CancelDispatchRequest {
    /// UUID of the dispatch to cancel.
    pub dispatch_id: Uuid,
    /// Reason for cancellation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Parse repeated transport `project_env` entries (`KEY=VALUE`) into the
/// canonical map shape used by [`CreateDispatchRequest`].
///
/// This parser enforces duplicate-key rejection so transport adapters do not
/// apply divergent overwrite semantics.
///
/// # Errors
///
/// Returns [`ContractError::InvalidField`] when an entry is malformed or a key
/// appears more than once.
pub fn parse_project_env_entries(
    entries: Vec<String>,
) -> Result<HashMap<String, String>, ContractError> {
    let mut result = HashMap::with_capacity(entries.len());
    for raw in entries {
        let (key_raw, value_raw) =
            raw.split_once('=')
                .ok_or_else(|| ContractError::InvalidField {
                    field: "project_env".to_owned(),
                    reason: format!("expected KEY=VALUE entry, got `{raw}`"),
                })?;

        match result.entry(key_raw.to_owned()) {
            Entry::Vacant(slot) => {
                slot.insert(value_raw.to_owned());
            }
            Entry::Occupied(_) => {
                return Err(ContractError::InvalidField {
                    field: "project_env".to_owned(),
                    reason: format!("duplicate environment key `{key_raw}`"),
                });
            }
        }
    }
    Ok(result)
}
