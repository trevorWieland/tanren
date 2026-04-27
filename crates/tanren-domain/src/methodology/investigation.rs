//! Durable investigation and remediation records.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::{FindingId, InvestigationAttemptId, RootCauseId, SpecId};
use crate::methodology::check::{CheckKind, CheckScope};
use crate::methodology::evidence::investigation::{Confidence, RootCauseCategory};
use crate::methodology::phase_id::PhaseId;
use crate::validated::NonEmptyString;

/// Source check that triggered investigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvestigationSourceCheck {
    pub phase: PhaseId,
    pub kind: CheckKind,
    pub scope: CheckScope,
}

/// One typed root-cause hypothesis captured durably in the event stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvestigationRootCause {
    pub id: RootCauseId,
    pub description: NonEmptyString,
    pub confidence: Confidence,
    pub category: RootCauseCategory,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
}

/// One durable investigation attempt for a recurring failure fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InvestigationAttempt {
    pub id: InvestigationAttemptId,
    pub spec_id: SpecId,
    pub fingerprint: NonEmptyString,
    pub loop_index: u16,
    pub source_check: InvestigationSourceCheck,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_findings: Vec<FindingId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_causes: Vec<InvestigationRootCause>,
    pub recorded_at: DateTime<Utc>,
}
