//! Actor / tenant attribution for multi-tenant governance and audit.
//!
//! Every dispatch carries an [`ActorContext`] that identifies the
//! organization, user, and optional team / API key / project responsible
//! for the action. Policy and audit events use the same structure so the
//! decision trail is always attributable.

use serde::{Deserialize, Serialize};

use crate::ids::{ApiKeyId, OrgId, ProjectId, TeamId, UserId};

/// Immutable attribution record for a dispatch or policy decision.
///
/// The org scope is always required; team, API key, and project are
/// optional in cases where they are not applicable (e.g. CLI invocations
/// outside any project).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorContext {
    /// Organization boundary — the top-level multi-tenant scope.
    pub org_id: OrgId,
    /// The individual user initiating the action.
    pub user_id: UserId,
    /// The team the user is acting on behalf of, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<TeamId>,
    /// The API key used to authenticate the caller, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<ApiKeyId>,
    /// The project scope for the action, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
}

impl ActorContext {
    /// Construct a minimal actor context with only org and user.
    #[must_use]
    pub const fn new(org_id: OrgId, user_id: UserId) -> Self {
        Self {
            org_id,
            user_id,
            team_id: None,
            api_key_id: None,
            project_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actor_serde_roundtrip() {
        let actor = ActorContext {
            org_id: OrgId::new(),
            user_id: UserId::new(),
            team_id: Some(TeamId::new()),
            api_key_id: Some(ApiKeyId::new()),
            project_id: Some(ProjectId::new()),
        };
        let json = serde_json::to_string(&actor).expect("serialize");
        let back: ActorContext = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(actor, back);
    }

    #[test]
    fn actor_minimal_skips_optional_fields() {
        let actor = ActorContext::new(OrgId::new(), UserId::new());
        let json = serde_json::to_string(&actor).expect("serialize");
        assert!(!json.contains("team_id"));
        assert!(!json.contains("api_key_id"));
        assert!(!json.contains("project_id"));
    }
}
