//! Wire contract for `record_rubric_score` and
//! `record_non_negotiable_compliance` (§3.2).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::pillar::{PillarId, PillarScope, PillarScore};
use tanren_domain::methodology::rubric::ComplianceStatus;
use tanren_domain::{FindingId, SpecId};

use super::SchemaVersion;

/// `record_rubric_score` params. Invariant enforcement (score < target
/// ⇒ findings; score < passing ⇒ `fix_now`) is applied in the service
/// layer via `tanren_domain::methodology::rubric::RubricScore::try_new`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordRubricScoreParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub scope: PillarScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope_target_id: Option<String>,
    pub pillar: PillarId,
    pub score: PillarScore,
    pub target: PillarScore,
    pub passing: PillarScore,
    pub rationale: String,
    #[serde(default)]
    pub supporting_finding_ids: Vec<FindingId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `record_non_negotiable_compliance` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordNonNegotiableComplianceParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub scope: PillarScope,
    pub name: String,
    pub status: ComplianceStatus,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
