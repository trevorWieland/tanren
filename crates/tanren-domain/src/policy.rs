//! Typed policy decision records.
//!
//! Policy decisions are first-class audit records, not opaque strings.
//! Every [`PolicyDecisionRecord`] identifies the decision kind, the
//! resource being governed, the scope applying the rule, and the outcome.

use serde::{Deserialize, Serialize};

use crate::actor::ActorContext;
use crate::ids::{DispatchId, LeaseId, OrgId, ProjectId, StepId, TeamId};

/// A fully attributed policy decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecisionRecord {
    /// What kind of policy produced this decision.
    pub kind: PolicyDecisionKind,
    /// The resource the decision applies to.
    pub resource: PolicyResourceRef,
    /// The tenant scope evaluated for this decision.
    pub scope: PolicyScope,
    /// Whether the action was allowed or denied.
    pub outcome: PolicyOutcome,
    /// Human-readable explanation for logs and UIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The class of policy that produced a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecisionKind {
    /// Authorization check (RBAC, ownership, capability).
    Authz,
    /// Quota check (rate limits, concurrent-job limits).
    Quota,
    /// Budget check (cost ceilings, token caps).
    Budget,
    /// Placement decision (substrate selection, lane routing).
    Placement,
}

impl std::fmt::Display for PolicyDecisionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authz => f.write_str("authz"),
            Self::Quota => f.write_str("quota"),
            Self::Budget => f.write_str("budget"),
            Self::Placement => f.write_str("placement"),
        }
    }
}

/// The outcome of a policy evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyOutcome {
    Allowed,
    Denied,
}

impl std::fmt::Display for PolicyOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allowed => f.write_str("allowed"),
            Self::Denied => f.write_str("denied"),
        }
    }
}

/// Typed reference to the resource a policy decision applies to.
///
/// Replaces the former stringly-typed `{resource_type, resource_id}`
/// struct so the set of valid policy resources matches the domain
/// surface at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyResourceRef {
    Dispatch {
        dispatch_id: DispatchId,
    },
    Step {
        dispatch_id: DispatchId,
        step_id: StepId,
    },
    Lease {
        dispatch_id: DispatchId,
        lease_id: LeaseId,
    },
    Org {
        org_id: OrgId,
    },
    Team {
        org_id: OrgId,
        team_id: TeamId,
    },
    Project {
        project_id: ProjectId,
    },
    /// A named budget envelope scoped to an org.
    Budget {
        org_id: OrgId,
        envelope: String,
    },
    /// A named quota bucket scoped to an org.
    Quota {
        org_id: OrgId,
        resource: String,
    },
}

/// The tenant scope evaluated in a policy decision.
///
/// Reuses [`ActorContext`] directly so policy decisions are always
/// attributable to the same org/user/team hierarchy that audit events use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PolicyScope(pub ActorContext);

impl PolicyScope {
    /// Construct a scope from an [`ActorContext`].
    #[must_use]
    pub const fn new(actor: ActorContext) -> Self {
        Self(actor)
    }

    /// Borrow the underlying actor context.
    #[must_use]
    pub const fn actor(&self) -> &ActorContext {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{OrgId, UserId};

    #[test]
    fn decision_record_serde_roundtrip() {
        let record = PolicyDecisionRecord {
            kind: PolicyDecisionKind::Budget,
            resource: PolicyResourceRef::Dispatch {
                dispatch_id: DispatchId::new(),
            },
            scope: PolicyScope::new(ActorContext::new(OrgId::new(), UserId::new())),
            outcome: PolicyOutcome::Allowed,
            reason: Some("within monthly budget".into()),
        };
        let json = serde_json::to_string(&record).expect("serialize");
        let back: PolicyDecisionRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(record, back);
    }

    #[test]
    fn resource_ref_tagged_format() {
        let r = PolicyResourceRef::Step {
            dispatch_id: DispatchId::new(),
            step_id: StepId::new(),
        };
        let json = serde_json::to_string(&r).expect("serialize");
        assert!(json.contains("\"type\":\"step\""));
        assert!(json.contains("\"dispatch_id\""));
        assert!(json.contains("\"step_id\""));
    }

    #[test]
    fn resource_ref_budget_envelope() {
        let r = PolicyResourceRef::Budget {
            org_id: OrgId::new(),
            envelope: "monthly-llm".into(),
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let back: PolicyResourceRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
    }
}
