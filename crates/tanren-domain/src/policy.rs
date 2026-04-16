//! Typed policy decision records.
//!
//! Policy decisions are first-class audit records, not opaque strings.
//! Every [`PolicyDecisionRecord`] identifies the decision kind, the
//! resource being governed, the scope applying the rule, and the outcome.

use serde::{Deserialize, Serialize};

use crate::actor::ActorContext;
use crate::ids::{ApiKeyId, DispatchId, LeaseId, OrgId, ProjectId, StepId, TeamId};

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
    /// Machine-readable policy reason code when an action is denied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<PolicyReasonCode>,
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

/// Typed machine-readable reason codes produced by policy checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyReasonCode {
    TimeoutOutOfRange,
    ProjectEnvTooLarge,
    ProjectEnvValueTooLong,
    RequiredSecretsTooLarge,
    SecretNameTooLong,
    TeamScopeRequiresProject,
    ApiKeyScopeRequiresProject,
    PreserveOnFailureRequiresManualMode,
    PhaseCliModeDisallowed,
    CancelOrgMismatch,
    CancelProjectScopeMismatch,
    CancelTeamScopeMismatch,
    CancelApiKeyScopeMismatch,
    CancelDispatchNotFound,
    ReadOrgMismatch,
    ReadProjectScopeMismatch,
    ReadTeamScopeMismatch,
    ReadApiKeyScopeMismatch,
}

impl std::fmt::Display for PolicyReasonCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::TimeoutOutOfRange => "timeout_out_of_range",
            Self::ProjectEnvTooLarge => "project_env_too_large",
            Self::ProjectEnvValueTooLong => "project_env_value_too_long",
            Self::RequiredSecretsTooLarge => "required_secrets_too_large",
            Self::SecretNameTooLong => "secret_name_too_long",
            Self::TeamScopeRequiresProject => "team_scope_requires_project",
            Self::ApiKeyScopeRequiresProject => "api_key_scope_requires_project",
            Self::PreserveOnFailureRequiresManualMode => "preserve_on_failure_requires_manual_mode",
            Self::PhaseCliModeDisallowed => "phase_cli_mode_disallowed",
            Self::CancelOrgMismatch => "cancel_org_mismatch",
            Self::CancelProjectScopeMismatch => "cancel_project_scope_mismatch",
            Self::CancelTeamScopeMismatch => "cancel_team_scope_mismatch",
            Self::CancelApiKeyScopeMismatch => "cancel_api_key_scope_mismatch",
            Self::CancelDispatchNotFound => "cancel_dispatch_not_found",
            Self::ReadOrgMismatch => "read_org_mismatch",
            Self::ReadProjectScopeMismatch => "read_project_scope_mismatch",
            Self::ReadTeamScopeMismatch => "read_team_scope_mismatch",
            Self::ReadApiKeyScopeMismatch => "read_api_key_scope_mismatch",
        };
        f.write_str(text)
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

/// Store-queryable read scope derived from trusted actor context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchReadScope {
    pub org_id: OrgId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<TeamId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_id: Option<ApiKeyId>,
}

impl DispatchReadScope {
    #[must_use]
    pub const fn from_actor(actor: &ActorContext) -> Self {
        Self {
            org_id: actor.org_id,
            project_id: actor.project_id,
            team_id: actor.team_id,
            api_key_id: actor.api_key_id,
        }
    }
}

/// Scope dimension that caused a read/cancel scope mismatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchScopeMismatch {
    Org,
    Project,
    Team,
    ApiKey,
}

/// Evaluate actor-vs-dispatch scope compatibility using null-or-exact semantics.
///
/// Rules:
/// - org must always match
/// - if dispatch has `project_id`/`team_id`/`api_key_id`, actor must match it
/// - actor may be more specific than dispatch (dispatch `None` is globally visible
///   within its org for that dimension)
pub fn actor_matches_dispatch_scope(
    actor: &ActorContext,
    dispatch_actor: &ActorContext,
) -> Result<(), DispatchScopeMismatch> {
    if actor.org_id != dispatch_actor.org_id {
        return Err(DispatchScopeMismatch::Org);
    }
    if dispatch_actor.project_id.is_some() && actor.project_id != dispatch_actor.project_id {
        return Err(DispatchScopeMismatch::Project);
    }
    if dispatch_actor.team_id.is_some() && actor.team_id != dispatch_actor.team_id {
        return Err(DispatchScopeMismatch::Team);
    }
    if dispatch_actor.api_key_id.is_some() && actor.api_key_id != dispatch_actor.api_key_id {
        return Err(DispatchScopeMismatch::ApiKey);
    }
    Ok(())
}

/// Returns whether a scoped reader is allowed to read a dispatch actor row.
#[must_use]
pub fn read_scope_allows_dispatch_actor(
    read_scope: DispatchReadScope,
    dispatch_actor: &ActorContext,
) -> bool {
    read_scope.org_id == dispatch_actor.org_id
        && scope_dimension_allows(dispatch_actor.project_id, read_scope.project_id)
        && scope_dimension_allows(dispatch_actor.team_id, read_scope.team_id)
        && scope_dimension_allows(dispatch_actor.api_key_id, read_scope.api_key_id)
}

fn scope_dimension_allows<T: Eq + Copy>(
    dispatch_scope: Option<T>,
    reader_scope: Option<T>,
) -> bool {
    reader_scope.map_or(dispatch_scope.is_none(), |reader| {
        dispatch_scope.is_none() || dispatch_scope == Some(reader)
    })
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
            reason_code: None,
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

    #[test]
    fn reason_code_serde_uses_snake_case() {
        let code = PolicyReasonCode::ReadOrgMismatch;
        let json = serde_json::to_string(&code).expect("serialize");
        assert_eq!(json, "\"read_org_mismatch\"");
        let back: PolicyReasonCode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, code);
    }

    #[test]
    fn actor_scope_match_requires_org_and_present_dispatch_dimensions() {
        let org = OrgId::new();
        let project = ProjectId::new();
        let team = TeamId::new();
        let api_key = ApiKeyId::new();
        let dispatch_actor = ActorContext {
            org_id: org,
            user_id: UserId::new(),
            project_id: Some(project),
            team_id: Some(team),
            api_key_id: Some(api_key),
        };
        let actor = ActorContext {
            org_id: org,
            user_id: UserId::new(),
            project_id: Some(project),
            team_id: Some(team),
            api_key_id: Some(api_key),
        };
        assert_eq!(
            actor_matches_dispatch_scope(&actor, &dispatch_actor),
            Ok(())
        );
    }

    #[test]
    fn actor_scope_match_rejects_dimension_mismatches() {
        let org = OrgId::new();
        let dispatch_actor = ActorContext {
            org_id: org,
            user_id: UserId::new(),
            project_id: Some(ProjectId::new()),
            team_id: Some(TeamId::new()),
            api_key_id: Some(ApiKeyId::new()),
        };
        let wrong_org = ActorContext::new(OrgId::new(), UserId::new());
        assert_eq!(
            actor_matches_dispatch_scope(&wrong_org, &dispatch_actor),
            Err(DispatchScopeMismatch::Org)
        );
        let wrong_project = ActorContext {
            org_id: org,
            user_id: UserId::new(),
            project_id: Some(ProjectId::new()),
            team_id: dispatch_actor.team_id,
            api_key_id: dispatch_actor.api_key_id,
        };
        assert_eq!(
            actor_matches_dispatch_scope(&wrong_project, &dispatch_actor),
            Err(DispatchScopeMismatch::Project)
        );
        let wrong_team = ActorContext {
            org_id: org,
            user_id: UserId::new(),
            project_id: dispatch_actor.project_id,
            team_id: Some(TeamId::new()),
            api_key_id: dispatch_actor.api_key_id,
        };
        assert_eq!(
            actor_matches_dispatch_scope(&wrong_team, &dispatch_actor),
            Err(DispatchScopeMismatch::Team)
        );
        let wrong_api_key = ActorContext {
            org_id: org,
            user_id: UserId::new(),
            project_id: dispatch_actor.project_id,
            team_id: dispatch_actor.team_id,
            api_key_id: Some(ApiKeyId::new()),
        };
        assert_eq!(
            actor_matches_dispatch_scope(&wrong_api_key, &dispatch_actor),
            Err(DispatchScopeMismatch::ApiKey)
        );
    }

    #[test]
    fn read_scope_allows_unscoped_dispatch_dimension_for_scoped_reader() {
        let org = OrgId::new();
        let read_scope = DispatchReadScope {
            org_id: org,
            project_id: Some(ProjectId::new()),
            team_id: Some(TeamId::new()),
            api_key_id: Some(ApiKeyId::new()),
        };
        let dispatch_actor = ActorContext {
            org_id: org,
            user_id: UserId::new(),
            project_id: None,
            team_id: None,
            api_key_id: None,
        };
        assert!(read_scope_allows_dispatch_actor(
            read_scope,
            &dispatch_actor
        ));
    }
}
