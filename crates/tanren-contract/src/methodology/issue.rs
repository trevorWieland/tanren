//! Wire contract for `create_issue` (§3.7).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::methodology::issue::{IssuePriority, IssueRef};
use tanren_domain::{IssueId, SpecId};

/// `create_issue` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateIssueParams {
    pub origin_spec_id: SpecId,
    pub title: String,
    pub description: String,
    pub suggested_spec_scope: String,
    pub priority: IssuePriority,
}

/// `create_issue` response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CreateIssueResponse {
    pub issue_id: IssueId,
    pub reference: IssueRef,
}
