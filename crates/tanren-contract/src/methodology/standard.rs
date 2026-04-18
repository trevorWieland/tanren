//! Wire contract for standards tools (§3.8).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::SpecId;

/// `list_relevant_standards` params.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ListRelevantStandardsParams {
    pub spec_id: SpecId,
}
