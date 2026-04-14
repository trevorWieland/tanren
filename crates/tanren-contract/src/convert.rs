//! Conversion impls between contract request types and domain commands.
//!
//! Validation happens here: empty strings are rejected and timeouts are
//! bounds-checked. Enum fields are already typed in the contract, so no
//! string parsing is needed.

use tanren_domain::{
    ActorContext, ApiKeyId, CancelDispatch, ConfigEnv, CreateDispatch, DispatchId, NonEmptyString,
    OrgId, ProjectId, TeamId, TimeoutSecs, UserId,
};

use crate::enums::{
    AuthMode as ContractAuthMode, Cli as ContractCli, DispatchMode as ContractDispatchMode,
    DispatchStatus as ContractDispatchStatus, Lane as ContractLane, Outcome as ContractOutcome,
    Phase as ContractPhase,
};
use crate::error::ContractError;
use crate::request::{CancelDispatchRequest, CreateDispatchRequest};

impl From<ContractPhase> for tanren_domain::Phase {
    fn from(value: ContractPhase) -> Self {
        match value {
            ContractPhase::DoTask => Self::DoTask,
            ContractPhase::AuditTask => Self::AuditTask,
            ContractPhase::RunDemo => Self::RunDemo,
            ContractPhase::AuditSpec => Self::AuditSpec,
            ContractPhase::Investigate => Self::Investigate,
            ContractPhase::Gate => Self::Gate,
            ContractPhase::Setup => Self::Setup,
            ContractPhase::Cleanup => Self::Cleanup,
        }
    }
}

impl From<tanren_domain::Phase> for ContractPhase {
    fn from(value: tanren_domain::Phase) -> Self {
        match value {
            tanren_domain::Phase::DoTask => Self::DoTask,
            tanren_domain::Phase::AuditTask => Self::AuditTask,
            tanren_domain::Phase::RunDemo => Self::RunDemo,
            tanren_domain::Phase::AuditSpec => Self::AuditSpec,
            tanren_domain::Phase::Investigate => Self::Investigate,
            tanren_domain::Phase::Gate => Self::Gate,
            tanren_domain::Phase::Setup => Self::Setup,
            tanren_domain::Phase::Cleanup => Self::Cleanup,
        }
    }
}

impl From<ContractCli> for tanren_domain::Cli {
    fn from(value: ContractCli) -> Self {
        match value {
            ContractCli::Claude => Self::Claude,
            ContractCli::Codex => Self::Codex,
            ContractCli::OpenCode => Self::OpenCode,
            ContractCli::Bash => Self::Bash,
        }
    }
}

impl From<tanren_domain::Cli> for ContractCli {
    fn from(value: tanren_domain::Cli) -> Self {
        match value {
            tanren_domain::Cli::Claude => Self::Claude,
            tanren_domain::Cli::Codex => Self::Codex,
            tanren_domain::Cli::OpenCode => Self::OpenCode,
            tanren_domain::Cli::Bash => Self::Bash,
        }
    }
}

impl From<ContractDispatchMode> for tanren_domain::DispatchMode {
    fn from(value: ContractDispatchMode) -> Self {
        match value {
            ContractDispatchMode::Auto => Self::Auto,
            ContractDispatchMode::Manual => Self::Manual,
        }
    }
}

impl From<tanren_domain::DispatchMode> for ContractDispatchMode {
    fn from(value: tanren_domain::DispatchMode) -> Self {
        match value {
            tanren_domain::DispatchMode::Auto => Self::Auto,
            tanren_domain::DispatchMode::Manual => Self::Manual,
        }
    }
}

impl From<ContractAuthMode> for tanren_domain::AuthMode {
    fn from(value: ContractAuthMode) -> Self {
        match value {
            ContractAuthMode::ApiKey => Self::ApiKey,
            ContractAuthMode::OAuth => Self::OAuth,
            ContractAuthMode::Subscription => Self::Subscription,
        }
    }
}

impl From<tanren_domain::AuthMode> for ContractAuthMode {
    fn from(value: tanren_domain::AuthMode) -> Self {
        match value {
            tanren_domain::AuthMode::ApiKey => Self::ApiKey,
            tanren_domain::AuthMode::OAuth => Self::OAuth,
            tanren_domain::AuthMode::Subscription => Self::Subscription,
        }
    }
}

impl From<ContractDispatchStatus> for tanren_domain::DispatchStatus {
    fn from(value: ContractDispatchStatus) -> Self {
        match value {
            ContractDispatchStatus::Pending => Self::Pending,
            ContractDispatchStatus::Running => Self::Running,
            ContractDispatchStatus::Completed => Self::Completed,
            ContractDispatchStatus::Failed => Self::Failed,
            ContractDispatchStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<tanren_domain::DispatchStatus> for ContractDispatchStatus {
    fn from(value: tanren_domain::DispatchStatus) -> Self {
        match value {
            tanren_domain::DispatchStatus::Pending => Self::Pending,
            tanren_domain::DispatchStatus::Running => Self::Running,
            tanren_domain::DispatchStatus::Completed => Self::Completed,
            tanren_domain::DispatchStatus::Failed => Self::Failed,
            tanren_domain::DispatchStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<ContractLane> for tanren_domain::Lane {
    fn from(value: ContractLane) -> Self {
        match value {
            ContractLane::Impl => Self::Impl,
            ContractLane::Audit => Self::Audit,
            ContractLane::Gate => Self::Gate,
        }
    }
}

impl From<tanren_domain::Lane> for ContractLane {
    fn from(value: tanren_domain::Lane) -> Self {
        match value {
            tanren_domain::Lane::Impl => Self::Impl,
            tanren_domain::Lane::Audit => Self::Audit,
            tanren_domain::Lane::Gate => Self::Gate,
        }
    }
}

impl From<ContractOutcome> for tanren_domain::Outcome {
    fn from(value: ContractOutcome) -> Self {
        match value {
            ContractOutcome::Success => Self::Success,
            ContractOutcome::Fail => Self::Fail,
            ContractOutcome::Blocked => Self::Blocked,
            ContractOutcome::Error => Self::Error,
            ContractOutcome::Timeout => Self::Timeout,
        }
    }
}

impl From<tanren_domain::Outcome> for ContractOutcome {
    fn from(value: tanren_domain::Outcome) -> Self {
        match value {
            tanren_domain::Outcome::Success => Self::Success,
            tanren_domain::Outcome::Fail => Self::Fail,
            tanren_domain::Outcome::Blocked => Self::Blocked,
            tanren_domain::Outcome::Error => Self::Error,
            tanren_domain::Outcome::Timeout => Self::Timeout,
        }
    }
}

impl TryFrom<CreateDispatchRequest> for CreateDispatch {
    type Error = ContractError;

    fn try_from(req: CreateDispatchRequest) -> Result<Self, Self::Error> {
        let actor = ActorContext {
            org_id: OrgId::from_uuid(req.org_id),
            user_id: UserId::from_uuid(req.user_id),
            team_id: req.team_id.map(TeamId::from_uuid),
            api_key_id: req.api_key_id.map(ApiKeyId::from_uuid),
            project_id: req.project_id.map(ProjectId::from_uuid),
        };

        let project = try_non_empty("project", req.project)?;
        let branch = try_non_empty("branch", req.branch)?;
        let spec_folder = try_non_empty("spec_folder", req.spec_folder)?;
        let workflow_id = try_non_empty("workflow_id", req.workflow_id)?;
        let environment_profile = try_non_empty("environment_profile", req.environment_profile)?;

        let timeout =
            TimeoutSecs::try_new(req.timeout_secs).map_err(|e| ContractError::InvalidField {
                field: "timeout_secs".to_owned(),
                reason: e.to_string(),
            })?;

        let project_env = match req.project_env {
            Some(map) => ConfigEnv::from(map),
            None => ConfigEnv::default(),
        };

        Ok(CreateDispatch {
            actor,
            project,
            phase: req.phase.into(),
            cli: req.cli.into(),
            auth_mode: req.auth_mode.unwrap_or(ContractAuthMode::ApiKey).into(),
            branch,
            spec_folder,
            workflow_id,
            mode: req.mode.into(),
            timeout,
            environment_profile,
            gate_cmd: req.gate_cmd,
            context: req.context,
            model: req.model,
            project_env,
            required_secrets: req.required_secrets.unwrap_or_default(),
            preserve_on_failure: req.preserve_on_failure.unwrap_or(false),
        })
    }
}

impl TryFrom<CancelDispatchRequest> for CancelDispatch {
    type Error = ContractError;

    fn try_from(req: CancelDispatchRequest) -> Result<Self, Self::Error> {
        let actor = ActorContext {
            org_id: OrgId::from_uuid(req.org_id),
            user_id: UserId::from_uuid(req.user_id),
            team_id: req.team_id.map(TeamId::from_uuid),
            api_key_id: req.api_key_id.map(ApiKeyId::from_uuid),
            project_id: req.project_id.map(ProjectId::from_uuid),
        };

        Ok(Self {
            actor,
            dispatch_id: DispatchId::from_uuid(req.dispatch_id),
            reason: req.reason,
        })
    }
}

/// Validate that a string is non-empty and wrap it in [`NonEmptyString`].
fn try_non_empty(field: &'static str, value: String) -> Result<NonEmptyString, ContractError> {
    NonEmptyString::try_new(value).map_err(|e| ContractError::InvalidField {
        field: field.to_owned(),
        reason: e.to_string(),
    })
}
