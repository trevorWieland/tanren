//! Wire contract for `create_issue` (§3.7).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::issue::{IssuePriority, IssueRef};
use tanren_domain::{IssueId, SpecId};

use super::SchemaVersion;

/// `create_issue` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateIssueParams {
    pub schema_version: SchemaVersion,
    pub origin_spec_id: SpecId,
    pub title: String,
    pub description: String,
    pub suggested_spec_scope: String,
    pub priority: IssuePriority,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// `create_issue` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateIssueResponse {
    pub schema_version: SchemaVersion,
    pub issue_id: IssueId,
    pub reference: IssueRef,
}
