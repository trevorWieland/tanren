//! Wire contract for generic check recording.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::ids::{CheckRunId, FindingId, SpecId};
use tanren_domain::methodology::check::{CheckKind, CheckScope, CheckStatus};

use super::SchemaVersion;

/// `check start` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartCheckRunParams {
    pub schema_version: SchemaVersion,
    pub spec_id: SpecId,
    pub kind: CheckKind,
    pub scope: CheckScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `check start` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartCheckRunResponse {
    pub schema_version: SchemaVersion,
    pub check_run_id: CheckRunId,
}

/// `check result` / `check failure` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecordCheckResultParams {
    pub schema_version: SchemaVersion,
    pub check_run_id: CheckRunId,
    pub spec_id: SpecId,
    pub kind: CheckKind,
    pub scope: CheckScope,
    pub status: CheckStatus,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finding_ids: Vec<FindingId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}
