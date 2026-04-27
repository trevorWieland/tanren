//! Generic check records for gate, audit, adherence, demo, and future checks.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{CheckRunId, FindingId, SpecId, TaskId};
use crate::methodology::phase_id::PhaseId;
use crate::validated::NonEmptyString;

/// Extensible check identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CheckKind {
    Gate,
    Audit,
    Adherence,
    Demo,
    SpecGate,
    Custom { name: NonEmptyString },
}

impl CheckKind {
    /// Stable display tag.
    #[must_use]
    pub fn tag(&self) -> &str {
        match self {
            Self::Gate => "gate",
            Self::Audit => "audit",
            Self::Adherence => "adherence",
            Self::Demo => "demo",
            Self::SpecGate => "spec_gate",
            Self::Custom { name } => name.as_str(),
        }
    }
}

/// Scope evaluated by a check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum CheckScope {
    Spec,
    Task { task_id: TaskId },
}

impl CheckScope {
    /// Return the task id when this is task-scoped.
    #[must_use]
    pub const fn task_id(&self) -> Option<TaskId> {
        match self {
            Self::Spec => None,
            Self::Task { task_id } => Some(*task_id),
        }
    }
}

/// Check status recorded as a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Fail,
}

/// A running or completed check run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckRun {
    pub id: CheckRunId,
    pub spec_id: SpecId,
    pub kind: CheckKind,
    pub scope: CheckScope,
    pub source_phase: PhaseId,
    pub fingerprint: Option<NonEmptyString>,
    pub started_at: DateTime<Utc>,
}

/// Result evidence for a check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CheckResult {
    pub run_id: CheckRunId,
    pub spec_id: SpecId,
    pub kind: CheckKind,
    pub scope: CheckScope,
    pub status: CheckStatus,
    pub summary: NonEmptyString,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finding_ids: Vec<FindingId>,
    pub recorded_at: DateTime<Utc>,
}
