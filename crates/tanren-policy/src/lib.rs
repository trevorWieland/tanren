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
//! - Every decision carries a deterministic reason code for audit
//! - The policy engine is synchronous — no I/O in policy evaluation

use tanren_domain::{
    ActorContext, CancelDispatch, Cli, CreateDispatch, DispatchId, DispatchMode, DispatchReadScope,
    DispatchScopeMismatch, DomainError, Phase, PolicyDecisionKind, PolicyDecisionRecord,
    PolicyOutcome, PolicyReasonCode, PolicyResourceRef, PolicyScope, actor_matches_dispatch_scope,
};

/// Security and admission limits for dispatch creation policy.
#[derive(Debug, Clone, Copy)]
pub struct PolicyLimits {
    pub min_timeout_secs: u64,
    pub max_timeout_secs: u64,
    pub max_project_env_entries: usize,
    pub max_project_env_value_len: usize,
    pub max_required_secrets: usize,
    pub max_secret_name_len: usize,
}

impl Default for PolicyLimits {
    fn default() -> Self {
        Self {
            min_timeout_secs: 30,
            max_timeout_secs: 3_600,
            max_project_env_entries: 64,
            max_project_env_value_len: 4_096,
            max_required_secrets: 32,
            max_secret_name_len: 128,
        }
    }
}

/// Fail-closed policy engine.
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    limits: PolicyLimits,
}

impl PolicyEngine {
    /// Create a new policy engine with strict default limits.
    #[must_use]
    pub fn new() -> Self {
        Self {
            limits: PolicyLimits::default(),
        }
    }

    /// Create a policy engine with custom limits.
    #[must_use]
    pub const fn with_limits(limits: PolicyLimits) -> Self {
        Self { limits }
    }

    /// Evaluate whether dispatch creation is allowed.
    pub fn check_dispatch_allowed(
        &self,
        cmd: &CreateDispatch,
        dispatch_id: DispatchId,
    ) -> Result<PolicyDecisionRecord, DomainError> {
        let scope = PolicyScope::new(cmd.actor.clone());
        let resource = PolicyResourceRef::Dispatch { dispatch_id };

        if let Some(violation) = self.first_violation(cmd) {
            return Ok(Self::deny(
                violation.code,
                &violation.message,
                scope,
                resource,
            ));
        }

        Ok(PolicyDecisionRecord {
            kind: PolicyDecisionKind::Authz,
            resource,
            scope,
            outcome: PolicyOutcome::Allowed,
            reason_code: None,
            reason: Some("allowed".to_owned()),
        })
    }

    /// Evaluate whether dispatch cancellation is allowed.
    pub fn check_cancel_allowed(
        &self,
        cmd: &CancelDispatch,
        dispatch_actor: Option<&ActorContext>,
    ) -> Result<PolicyDecisionRecord, DomainError> {
        let scope = PolicyScope::new(cmd.actor.clone());
        let resource = PolicyResourceRef::Dispatch {
            dispatch_id: cmd.dispatch_id,
        };

        if let Some(violation) = Self::first_cancel_violation(cmd, dispatch_actor) {
            return Ok(Self::deny(
                violation.code,
                &violation.message,
                scope,
                resource,
            ));
        }

        Ok(PolicyDecisionRecord {
            kind: PolicyDecisionKind::Authz,
            resource,
            scope,
            outcome: PolicyOutcome::Allowed,
            reason_code: None,
            reason: Some("allowed".to_owned()),
        })
    }

    /// Build a store-queryable read scope from trusted actor context.
    #[must_use]
    pub const fn dispatch_read_scope(&self, actor: &ActorContext) -> DispatchReadScope {
        DispatchReadScope::from_actor(actor)
    }

    fn deny(
        code: PolicyReasonCode,
        message: &str,
        scope: PolicyScope,
        resource: PolicyResourceRef,
    ) -> PolicyDecisionRecord {
        PolicyDecisionRecord {
            kind: PolicyDecisionKind::Authz,
            resource,
            scope,
            outcome: PolicyOutcome::Denied,
            reason_code: Some(code),
            reason: Some(message.to_owned()),
        }
    }

    fn first_violation(&self, cmd: &CreateDispatch) -> Option<PolicyViolation> {
        self.check_timeout(cmd)
            .or_else(|| self.check_project_env(cmd))
            .or_else(|| self.check_required_secrets(cmd))
            .or_else(|| Self::check_actor_scope(cmd))
            .or_else(|| Self::check_preserve_on_failure(cmd))
            .or_else(|| Self::check_phase_cli_mode(cmd))
    }

    fn check_timeout(&self, cmd: &CreateDispatch) -> Option<PolicyViolation> {
        if self.timeout_allowed(cmd.timeout.get()) {
            None
        } else {
            Some(PolicyViolation::new(
                PolicyReasonCode::TimeoutOutOfRange,
                format!(
                    "timeout_secs must be in [{}..={}]",
                    self.limits.min_timeout_secs, self.limits.max_timeout_secs
                ),
            ))
        }
    }

    fn check_project_env(&self, cmd: &CreateDispatch) -> Option<PolicyViolation> {
        if cmd.project_env.as_map().len() > self.limits.max_project_env_entries {
            return Some(PolicyViolation::new(
                PolicyReasonCode::ProjectEnvTooLarge,
                format!(
                    "project_env entries must be <= {}",
                    self.limits.max_project_env_entries
                ),
            ));
        }
        for (_, value) in cmd.project_env.iter() {
            if value.len() > self.limits.max_project_env_value_len {
                return Some(PolicyViolation::new(
                    PolicyReasonCode::ProjectEnvValueTooLong,
                    format!(
                        "project_env value length must be <= {}",
                        self.limits.max_project_env_value_len
                    ),
                ));
            }
        }
        None
    }

    fn check_required_secrets(&self, cmd: &CreateDispatch) -> Option<PolicyViolation> {
        if cmd.required_secrets.len() > self.limits.max_required_secrets {
            return Some(PolicyViolation::new(
                PolicyReasonCode::RequiredSecretsTooLarge,
                format!(
                    "required_secrets length must be <= {}",
                    self.limits.max_required_secrets
                ),
            ));
        }

        for secret_name in &cmd.required_secrets {
            if secret_name.len() > self.limits.max_secret_name_len {
                return Some(PolicyViolation::new(
                    PolicyReasonCode::SecretNameTooLong,
                    format!(
                        "secret name length must be <= {}",
                        self.limits.max_secret_name_len
                    ),
                ));
            }
        }
        None
    }

    fn check_actor_scope(cmd: &CreateDispatch) -> Option<PolicyViolation> {
        if cmd.actor.team_id.is_some() && cmd.actor.project_id.is_none() {
            return Some(PolicyViolation::new(
                PolicyReasonCode::TeamScopeRequiresProject,
                "team-scoped actor must include project_id".to_owned(),
            ));
        }
        if cmd.actor.api_key_id.is_some() && cmd.actor.project_id.is_none() {
            return Some(PolicyViolation::new(
                PolicyReasonCode::ApiKeyScopeRequiresProject,
                "api-key actor must include project_id".to_owned(),
            ));
        }
        None
    }

    fn check_preserve_on_failure(cmd: &CreateDispatch) -> Option<PolicyViolation> {
        if cmd.preserve_on_failure && cmd.mode != DispatchMode::Manual {
            Some(PolicyViolation::new(
                PolicyReasonCode::PreserveOnFailureRequiresManualMode,
                "preserve_on_failure is only allowed for manual mode".to_owned(),
            ))
        } else {
            None
        }
    }

    fn check_phase_cli_mode(cmd: &CreateDispatch) -> Option<PolicyViolation> {
        if is_allowed_phase_cli_mode(cmd.phase, cmd.cli, cmd.mode) {
            None
        } else {
            Some(PolicyViolation::new(
                PolicyReasonCode::PhaseCliModeDisallowed,
                format!(
                    "phase={}, cli={}, mode={} is not permitted",
                    cmd.phase, cmd.cli, cmd.mode
                ),
            ))
        }
    }

    fn timeout_allowed(&self, timeout_secs: u64) -> bool {
        timeout_secs >= self.limits.min_timeout_secs && timeout_secs <= self.limits.max_timeout_secs
    }

    fn first_cancel_violation(
        cmd: &CancelDispatch,
        dispatch_actor: Option<&ActorContext>,
    ) -> Option<PolicyViolation> {
        let Some(dispatch_actor) = dispatch_actor else {
            return Some(PolicyViolation::new(
                PolicyReasonCode::CancelDispatchNotFound,
                "dispatch not found".to_owned(),
            ));
        };

        match actor_matches_dispatch_scope(&cmd.actor, dispatch_actor) {
            Ok(()) => None,
            Err(DispatchScopeMismatch::Org) => Some(PolicyViolation::new(
                PolicyReasonCode::CancelOrgMismatch,
                "actor org_id does not match dispatch org_id".to_owned(),
            )),
            Err(DispatchScopeMismatch::Project) => Some(PolicyViolation::new(
                PolicyReasonCode::CancelProjectScopeMismatch,
                "dispatch requires matching project_id".to_owned(),
            )),
            Err(DispatchScopeMismatch::Team) => Some(PolicyViolation::new(
                PolicyReasonCode::CancelTeamScopeMismatch,
                "dispatch requires matching team_id".to_owned(),
            )),
            Err(DispatchScopeMismatch::ApiKey) => Some(PolicyViolation::new(
                PolicyReasonCode::CancelApiKeyScopeMismatch,
                "dispatch requires matching api_key_id".to_owned(),
            )),
        }
    }
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn is_allowed_phase_cli_mode(phase: Phase, cli: Cli, mode: DispatchMode) -> bool {
    !matches!(
        (phase, cli, mode),
        (
            Phase::Setup | Phase::Cleanup | Phase::AuditSpec | Phase::Gate | Phase::Investigate,
            _,
            DispatchMode::Auto
        ) | (Phase::AuditSpec | Phase::Gate, Cli::Bash, _)
    )
}

#[derive(Debug, Clone)]
struct PolicyViolation {
    code: PolicyReasonCode,
    message: String,
}

impl PolicyViolation {
    fn new(code: PolicyReasonCode, message: String) -> Self {
        Self { code, message }
    }
}
