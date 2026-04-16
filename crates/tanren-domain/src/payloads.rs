//! Step input payloads and result payloads.
//!
//! # Secret handling
//!
//! Domain events are the canonical, persistable history. Types that live
//! inside events (or inside payloads that end up embedded in events) must
//! never carry credentials or caller-provided secret material.
//!
//! - [`ConfigEnv`] — command-only, full key/value env. **Not** embedded in
//!   [`DispatchSnapshot`]. `Debug` is redacted (keys only).
//! - [`ConfigKeys`] — the persistable audit-only projection of a
//!   [`ConfigEnv`]: sorted unique keys, no values.
//! - [`EnvironmentHandle`] — carries only an id and a runtime type name.
//!   Runtime-specific handle data lives in runtime-local storage, keyed
//!   by the handle id, and never crosses the domain boundary.
//!
//! # Output redaction contract
//!
//! [`ExecuteResult::tail_output`], [`ExecuteResult::stderr_tail`], and
//! [`ExecuteResult::gate_output`] are free-form strings captured from
//! the harness. The domain layer stores them verbatim — once serialized
//! into a `StepCompleted` event and persisted to the event log, any
//! secret they contain is effectively unrecoverable.
//!
//! **Harness adapters are responsible for redacting known secret
//! patterns before producing an `ExecuteResult`.** This includes:
//!
//! 1. API keys, bearer tokens, cookies, and session identifiers
//! 2. Values of environment variables listed in `required_secrets`
//!    on the dispatch snapshot
//! 3. Contents of files matching known credential path patterns
//!    (`~/.aws/credentials`, `~/.config/gcloud/*`, etc.)
//!
//! The domain crate cannot enforce this contract — it has no harness
//! context — so Phase 1 harness adapters MUST implement redaction at
//! the capture site. See `docs/rewrite/tasks/LANE-1.1-HARNESS.md` for
//! the adapter-side requirement.
//!
//! # Boxing
//!
//! Variants of [`StepPayload`] and [`StepResult`] are boxed because
//! `DispatchSnapshot` and `ExecuteResult` are large; inlining them would
//! make every clone pay for the largest shape.

use std::collections::{BTreeSet, HashMap};
use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::DispatchId;
use crate::status::{AuthMode, Cli, Outcome, Phase};
use crate::validated::{FiniteF64, NonEmptyString, TimeoutSecs};

// ---------------------------------------------------------------------------
// ConfigEnv — command-only, value-bearing, redacted Debug
// ---------------------------------------------------------------------------

/// Non-secret project configuration environment (command-only).
///
/// This type carries full key/value pairs and **must not** be embedded
/// in types that are persisted to the event log. `Debug` output emits
/// keys only; `Serialize` emits the full map because workers need the
/// values delivered out-of-band. Any orchestrator that persists an
/// environment to durable storage must do so through a runtime-local
/// store, never through a domain event.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigEnv(HashMap<String, String>);

impl ConfigEnv {
    /// Construct from a `HashMap`.
    #[must_use]
    pub const fn new(map: HashMap<String, String>) -> Self {
        Self(map)
    }

    /// Return `true` if the env is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over key/value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Borrow the inner map.
    #[must_use]
    pub const fn as_map(&self) -> &HashMap<String, String> {
        &self.0
    }

    /// Project into a keys-only [`ConfigKeys`] suitable for embedding in
    /// events or dispatch snapshots.
    #[must_use]
    pub fn to_keys(&self) -> ConfigKeys {
        ConfigKeys::from_strings(self.0.keys().cloned())
    }
}

impl fmt::Debug for ConfigEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Values must never be rendered — the set of keys only.
        let mut set = f.debug_set();
        for key in self.0.keys() {
            set.entry(&key);
        }
        set.finish()
    }
}

impl From<HashMap<String, String>> for ConfigEnv {
    fn from(map: HashMap<String, String>) -> Self {
        Self(map)
    }
}

// ---------------------------------------------------------------------------
// ConfigKeys — persistable, audit-only projection
// ---------------------------------------------------------------------------

/// Sorted, unique list of configuration keys.
///
/// This is the only env representation allowed inside [`DispatchSnapshot`]
/// (and therefore inside events). It carries no values at all, so
/// persisting it cannot leak secrets. Schedulers and workers fetch the
/// actual values from runtime-local config storage keyed by the dispatch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConfigKeys(Vec<String>);

impl ConfigKeys {
    /// Construct from an iterator of key strings. Duplicates are removed
    /// and the result is sorted for deterministic serialization.
    pub fn from_strings(iter: impl IntoIterator<Item = String>) -> Self {
        let set: BTreeSet<String> = iter.into_iter().collect();
        Self(set.into_iter().collect())
    }

    /// Return `true` if no keys are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Borrow the keys as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }

    /// Iterate over the keys.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// Dispatch snapshot — immutable resolved record
// ---------------------------------------------------------------------------

/// Immutable snapshot of a dispatch's resolved configuration.
///
/// Separate from [`crate::commands::CreateDispatch`] which represents
/// intent; a snapshot is the fully resolved, validated record embedded
/// in events and step payloads. **Contains no secret material** —
/// `project_env` is a keys-only list and `required_secrets` names the
/// secrets by reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchSnapshot {
    pub project: NonEmptyString,
    pub phase: Phase,
    pub cli: Cli,
    pub auth_mode: AuthMode,
    pub branch: NonEmptyString,
    pub spec_folder: NonEmptyString,
    pub workflow_id: NonEmptyString,
    pub timeout: TimeoutSecs,
    pub environment_profile: NonEmptyString,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_cmd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Audit-only projection of the caller's configuration environment.
    /// Values are **never** embedded in snapshots.
    #[serde(default, skip_serializing_if = "ConfigKeys::is_empty")]
    pub project_env: ConfigKeys,
    /// Names of secrets the orchestrator must inject at runtime.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_secrets: Vec<String>,
    pub preserve_on_failure: bool,
    pub created_at: DateTime<Utc>,
}

/// Opaque handle to a provisioned execution environment.
///
/// Domain types carry only the id and runtime tag. Runtime-specific
/// metadata (container IDs, VM tokens, volume handles) lives in a
/// runtime-local store keyed by `id` and never crosses the domain
/// boundary. This guarantees event payloads cannot leak credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentHandle {
    pub id: NonEmptyString,
    pub runtime_type: NonEmptyString,
}

/// Token usage counters from a harness execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
}

/// Severity level for a finding produced by audit or gate steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Fix,
    Note,
    Question,
}

impl fmt::Display for FindingSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fix => f.write_str("fix"),
            Self::Note => f.write_str("note"),
            Self::Question => f.write_str("question"),
        }
    }
}

/// A finding produced during audit or gate execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub title: String,
    pub description: String,
    pub severity: FindingSeverity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_numbers: Vec<u32>,
}

// ---------------------------------------------------------------------------
// Step input payloads (what the worker receives)
// ---------------------------------------------------------------------------

/// Payload for a provision step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionPayload {
    pub dispatch: DispatchSnapshot,
}

/// Typed reference to the canonical dispatch snapshot.
///
/// The dispatch snapshot is stored once in the dispatch projection and
/// creation event. Provision steps can carry this reference instead of
/// duplicating the full snapshot payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchSnapshotRef {
    pub dispatch_id: DispatchId,
}

impl DispatchSnapshotRef {
    #[must_use]
    pub const fn new(dispatch_id: DispatchId) -> Self {
        Self { dispatch_id }
    }
}

/// Snapshot-reference payload for a provision step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionRefPayload {
    pub dispatch_ref: DispatchSnapshotRef,
}

/// Payload for an execute step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutePayload {
    pub dispatch: DispatchSnapshot,
    pub handle: EnvironmentHandle,
}

/// Payload for a teardown step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeardownPayload {
    pub dispatch: DispatchSnapshot,
    pub handle: EnvironmentHandle,
    pub preserve: bool,
}

/// Payload for a dry-run step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DryRunPayload {
    pub dispatch: DispatchSnapshot,
}

/// Tagged union of step input payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepPayload {
    Provision(Box<ProvisionPayload>),
    ProvisionRef(Box<ProvisionRefPayload>),
    Execute(Box<ExecutePayload>),
    Teardown(Box<TeardownPayload>),
    DryRun(Box<DryRunPayload>),
}

// ---------------------------------------------------------------------------
// Step result payloads (what the worker produces)
// ---------------------------------------------------------------------------

/// Result of a provision step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvisionResult {
    pub handle: EnvironmentHandle,
}

/// Result of an execute step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub outcome: Outcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub duration_secs: FiniteF64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_tail: Option<String>,
    #[serde(default)]
    pub pushed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_hash: Option<String>,
    #[serde(default)]
    pub unchecked_tasks: u32,
    #[serde(default)]
    pub spec_modified: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<Finding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
}

/// Result of a teardown step.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TeardownResult {
    pub vm_released: bool,
    pub duration_secs: FiniteF64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost: Option<FiniteF64>,
}

/// Result of a dry-run step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DryRunResult {
    pub provider: String,
    pub server_type: String,
    pub estimated_cost_hourly: FiniteF64,
    pub would_provision: bool,
}

/// Tagged union of step result payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepResult {
    Provision(Box<ProvisionResult>),
    Execute(Box<ExecuteResult>),
    Teardown(Box<TeardownResult>),
    DryRun(Box<DryRunResult>),
}

#[cfg(test)]
mod tests;
