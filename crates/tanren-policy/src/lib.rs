//! Authorization and governance for the tanren control plane.
//!
//! Depends on: `tanren-domain`
//!
//! # Responsibilities
//!
//! - Authorization (RBAC, ownership, capability) checks
//! - Budget and quota enforcement
//! - Placement policy decisions
//! - Typed decision records for audit
//!
//! # Design Rules
//!
//! - Policy returns typed decisions, never transport-layer errors
//! - Every decision carries a reason string for audit
//! - The policy engine is synchronous — no I/O in policy evaluation
//!
//! # Lane 0.4
//!
//! This is a pass-through skeleton. All checks return `Allowed`.
//! Real policy logic (authz, budgets, quotas, placement) arrives in
//! Lane 0.5+. The skeleton exists so the orchestrator's signature
//! includes policy from day one, using the canonical domain types.

use tanren_domain::{
    CreateDispatch, DispatchId, DomainError, PolicyDecisionKind, PolicyDecisionRecord,
    PolicyOutcome, PolicyResourceRef, PolicyScope,
};

/// Pass-through policy engine.
///
/// Always permits all operations. Real policy enforcement arrives in
/// Lane 0.5 with identity scopes, budget ceilings, and placement rules.
#[derive(Debug, Clone)]
pub struct PolicyEngine;

impl PolicyEngine {
    /// Create a new pass-through policy engine.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Evaluate whether a dispatch creation is allowed.
    ///
    /// In this pass-through skeleton, always returns `PolicyOutcome::Allowed`.
    ///
    /// # Errors
    ///
    /// Returns `DomainError` if policy evaluation itself fails (not
    /// if the policy denies the request — that is indicated by
    /// `PolicyOutcome::Denied` in the returned record).
    pub fn check_dispatch_allowed(
        &self,
        cmd: &CreateDispatch,
        dispatch_id: DispatchId,
    ) -> Result<PolicyDecisionRecord, DomainError> {
        Ok(PolicyDecisionRecord {
            kind: PolicyDecisionKind::Authz,
            resource: PolicyResourceRef::Dispatch { dispatch_id },
            scope: PolicyScope::new(cmd.actor.clone()),
            outcome: PolicyOutcome::Allowed,
            reason: Some("pass-through: all dispatches allowed".to_owned()),
        })
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tanren_domain::{
        ActorContext, AuthMode, Cli, ConfigEnv, DispatchMode, NonEmptyString, OrgId, Phase,
        TimeoutSecs, UserId,
    };

    fn sample_command() -> CreateDispatch {
        CreateDispatch {
            actor: ActorContext::new(OrgId::new(), UserId::new()),
            project: NonEmptyString::try_new("test-project".to_owned()).expect("non-empty"),
            phase: Phase::DoTask,
            cli: Cli::Claude,
            auth_mode: AuthMode::ApiKey,
            branch: NonEmptyString::try_new("main".to_owned()).expect("non-empty"),
            spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("non-empty"),
            workflow_id: NonEmptyString::try_new("wf-1".to_owned()).expect("non-empty"),
            mode: DispatchMode::Manual,
            timeout: TimeoutSecs::try_new(60).expect("positive"),
            environment_profile: NonEmptyString::try_new("default".to_owned()).expect("non-empty"),
            gate_cmd: None,
            context: None,
            model: None,
            project_env: ConfigEnv::default(),
            required_secrets: vec![],
            preserve_on_failure: false,
        }
    }

    #[test]
    fn pass_through_allows_all() {
        let engine = PolicyEngine::new();
        let cmd = sample_command();
        let dispatch_id = DispatchId::new();
        let decision = engine
            .check_dispatch_allowed(&cmd, dispatch_id)
            .expect("policy should not error");
        assert_eq!(decision.outcome, PolicyOutcome::Allowed);
        assert_eq!(decision.kind, PolicyDecisionKind::Authz);
        assert!(decision.reason.is_some());
        assert!(matches!(
            decision.resource,
            PolicyResourceRef::Dispatch { dispatch_id: id } if id == dispatch_id
        ));
    }

    #[test]
    fn default_creates_engine() {
        let engine = PolicyEngine;
        let cmd = sample_command();
        let decision = engine
            .check_dispatch_allowed(&cmd, DispatchId::new())
            .expect("policy should not error");
        assert_eq!(decision.outcome, PolicyOutcome::Allowed);
    }
}
