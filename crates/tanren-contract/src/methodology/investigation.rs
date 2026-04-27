//! Wire contract for durable investigation/remediation records.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::ids::{FindingId, InvestigationAttemptId, RootCauseId, SpecId};
use tanren_domain::methodology::evidence::investigation::{Confidence, RootCauseCategory};
use tanren_domain::methodology::investigation::{InvestigationAttempt, InvestigationSourceCheck};

use super::SchemaVersion;

/// Root-cause input for `investigation record-attempt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InvestigationRootCauseInput {
    pub description: String,
    pub confidence: Confidence,
    pub category: RootCauseCategory,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub affected_files: Vec<String>,
}

/// `investigation record-attempt` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordInvestigationAttemptParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub fingerprint: String,
    pub loop_index: u16,
    pub source_check: InvestigationSourceCheck,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_findings: Vec<FindingId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_causes: Vec<InvestigationRootCauseInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `investigation record-attempt` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordInvestigationAttemptResponse {
    pub schema_version: SchemaVersion,
    pub attempt_id: InvestigationAttemptId,
    pub root_cause_ids: Vec<RootCauseId>,
}

/// `investigation list-attempts` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListInvestigationAttemptsParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_check: Option<InvestigationSourceCheck>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finding_id: Option<FindingId>,
}

/// `investigation list-attempts` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListInvestigationAttemptsResponse {
    pub schema_version: SchemaVersion,
    pub attempts: Vec<InvestigationAttempt>,
}

/// `investigation link-root-cause` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkRootCauseToFindingParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub attempt_id: InvestigationAttemptId,
    pub root_cause_id: RootCauseId,
    pub finding_id: FindingId,
    pub source_check: InvestigationSourceCheck,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
