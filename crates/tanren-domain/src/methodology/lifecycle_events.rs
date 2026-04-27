//! Finding, check, and investigation event payloads.

use serde::{Deserialize, Serialize};

use crate::ids::{FindingId, InvestigationAttemptId, RootCauseId, SpecId};
use crate::methodology::check::{CheckResult, CheckRun};
use crate::methodology::finding::{Finding, FindingLifecycleEvidence, StandardRef};
use crate::methodology::investigation::{InvestigationAttempt, InvestigationSourceCheck};
use crate::methodology::phase_id::PhaseId;

/// An audit or demo finding has been recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingAdded {
    pub finding: Box<Finding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// An adherence finding has been recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdherenceFindingAdded {
    pub finding: Box<Finding>,
    pub standard: StandardRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A finding has been independently verified as fixed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingResolved {
    pub finding_id: FindingId,
    pub spec_id: SpecId,
    pub evidence: FindingLifecycleEvidence,
    pub source_phase: PhaseId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A previously non-blocking finding has recurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingReopened {
    pub finding_id: FindingId,
    pub spec_id: SpecId,
    pub evidence: FindingLifecycleEvidence,
    pub source_phase: PhaseId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A finding has been explicitly deferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingDeferred {
    pub finding_id: FindingId,
    pub spec_id: SpecId,
    pub evidence: FindingLifecycleEvidence,
    pub source_phase: PhaseId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A finding has been replaced by more precise finding(s).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingSuperseded {
    pub finding_id: FindingId,
    pub spec_id: SpecId,
    pub superseded_by: Vec<FindingId>,
    pub evidence: FindingLifecycleEvidence,
    pub source_phase: PhaseId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A recheck observed that a finding is still present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingStillOpen {
    pub finding_id: FindingId,
    pub spec_id: SpecId,
    pub evidence: FindingLifecycleEvidence,
    pub source_phase: PhaseId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A generic check run started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRunStarted {
    pub check: CheckRun,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A generic check result was recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResultRecorded {
    pub result: CheckResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A generic check failure was recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckFailureRecorded {
    pub result: CheckResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A durable investigation attempt was recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvestigationAttemptRecorded {
    pub attempt: InvestigationAttempt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// A root cause was linked to a source finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootCauseLinkedToFinding {
    pub spec_id: SpecId,
    pub attempt_id: InvestigationAttemptId,
    pub root_cause_id: RootCauseId,
    pub finding_id: FindingId,
    pub source_check: InvestigationSourceCheck,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
