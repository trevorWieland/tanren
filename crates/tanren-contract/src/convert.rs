//! Conversion impls between contract request types and domain commands.
//!
//! Validation happens here: empty strings are rejected and timeouts are
//! bounds-checked. Enum fields are already typed in the contract, so no
//! string parsing is needed.

use std::collections::{HashMap, HashSet};

use tanren_domain::{
    ActorContext, CancelDispatch, ConfigEnv, CreateDispatch, DispatchId, NonEmptyString,
    TimeoutSecs,
};

use crate::enums::{
    AuthMode as ContractAuthMode, Cli as ContractCli, DispatchMode as ContractDispatchMode,
    DispatchStatus as ContractDispatchStatus, Lane as ContractLane, Outcome as ContractOutcome,
    Phase as ContractPhase, StepReadyState as ContractStepReadyState,
    StepStatus as ContractStepStatus, StepType as ContractStepType,
};
use crate::error::ContractError;
use crate::request::{CancelDispatchRequest, CreateDispatchRequest};

macro_rules! impl_enum_bimap {
    ($contract_type:path, $domain_type:path, { $($contract_variant:ident => $domain_variant:ident),+ $(,)? }) => {
        impl From<$contract_type> for $domain_type {
            fn from(value: $contract_type) -> Self {
                match value {
                    $(<$contract_type>::$contract_variant => <$domain_type>::$domain_variant,)+
                }
            }
        }

        impl From<$domain_type> for $contract_type {
            fn from(value: $domain_type) -> Self {
                match value {
                    $(<$domain_type>::$domain_variant => <$contract_type>::$contract_variant,)+
                }
            }
        }
    };
}

impl_enum_bimap!(ContractPhase, tanren_domain::Phase, {
    DoTask => DoTask,
    AuditTask => AuditTask,
    RunDemo => RunDemo,
    AuditSpec => AuditSpec,
    Investigate => Investigate,
    Gate => Gate,
    Setup => Setup,
    Cleanup => Cleanup,
});

impl_enum_bimap!(ContractCli, tanren_domain::Cli, {
    Claude => Claude,
    Codex => Codex,
    OpenCode => OpenCode,
    Bash => Bash,
});

impl_enum_bimap!(ContractDispatchMode, tanren_domain::DispatchMode, {
    Auto => Auto,
    Manual => Manual,
});

impl_enum_bimap!(ContractAuthMode, tanren_domain::AuthMode, {
    ApiKey => ApiKey,
    OAuth => OAuth,
    Subscription => Subscription,
});

impl_enum_bimap!(ContractDispatchStatus, tanren_domain::DispatchStatus, {
    Pending => Pending,
    Running => Running,
    Completed => Completed,
    Failed => Failed,
    Cancelled => Cancelled,
});

impl_enum_bimap!(ContractLane, tanren_domain::Lane, {
    Impl => Impl,
    Audit => Audit,
    Gate => Gate,
});

impl_enum_bimap!(ContractOutcome, tanren_domain::Outcome, {
    Success => Success,
    Fail => Fail,
    Blocked => Blocked,
    Error => Error,
    Timeout => Timeout,
});

impl_enum_bimap!(ContractStepType, tanren_domain::StepType, {
    Provision => Provision,
    Execute => Execute,
    Teardown => Teardown,
    DryRun => DryRun,
});

impl_enum_bimap!(ContractStepStatus, tanren_domain::StepStatus, {
    Pending => Pending,
    Running => Running,
    Completed => Completed,
    Failed => Failed,
    Cancelled => Cancelled,
});

impl_enum_bimap!(ContractStepReadyState, tanren_domain::StepReadyState, {
    Blocked => Blocked,
    Ready => Ready,
});

/// Validate that a string is non-empty and wrap it in [`NonEmptyString`].
fn try_non_empty(field: &'static str, value: String) -> Result<NonEmptyString, ContractError> {
    NonEmptyString::try_new(value).map_err(|e| ContractError::InvalidField {
        field: field.to_owned(),
        reason: e.to_string(),
    })
}

fn validate_project_env(project_env: &HashMap<String, String>) -> Result<(), ContractError> {
    for key in project_env.keys() {
        if !is_valid_env_key(key) {
            return Err(ContractError::InvalidField {
                field: "project_env".to_owned(),
                reason: format!("invalid environment key `{key}`"),
            });
        }
    }
    Ok(())
}

fn validate_required_secrets(required_secrets: &[String]) -> Result<(), ContractError> {
    let mut seen = HashSet::with_capacity(required_secrets.len());
    for secret_name in required_secrets {
        if secret_name.is_empty() {
            return Err(ContractError::InvalidField {
                field: "required_secrets".to_owned(),
                reason: "secret names must be non-empty".to_owned(),
            });
        }
        if !is_valid_secret_name(secret_name) {
            return Err(ContractError::InvalidField {
                field: "required_secrets".to_owned(),
                reason: format!("invalid secret name `{secret_name}`"),
            });
        }
        if !seen.insert(secret_name) {
            return Err(ContractError::InvalidField {
                field: "required_secrets".to_owned(),
                reason: format!("duplicate secret name `{secret_name}`"),
            });
        }
    }
    Ok(())
}

fn is_valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(ch) if ch == '_' || ch.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_valid_secret_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(ch) if ch == '_' || ch.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

/// Convert a trusted actor context plus create request into a domain command.
pub fn create_dispatch_from_request(
    actor: ActorContext,
    req: CreateDispatchRequest,
) -> Result<CreateDispatch, ContractError> {
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
    validate_project_env(&req.project_env)?;
    validate_required_secrets(&req.required_secrets)?;

    Ok(CreateDispatch {
        actor,
        project,
        phase: req.phase.into(),
        cli: req.cli.into(),
        auth_mode: req.auth_mode.into(),
        branch,
        spec_folder,
        workflow_id,
        mode: req.mode.into(),
        timeout,
        environment_profile,
        gate_cmd: req.gate_cmd,
        context: req.context,
        model: req.model,
        project_env: ConfigEnv::from(req.project_env),
        required_secrets: req.required_secrets,
        preserve_on_failure: req.preserve_on_failure,
    })
}

/// Convert a trusted actor context plus cancel request into a domain command.
pub fn cancel_dispatch_from_request(
    actor: ActorContext,
    req: CancelDispatchRequest,
) -> Result<CancelDispatch, ContractError> {
    Ok(CancelDispatch {
        actor,
        dispatch_id: DispatchId::from_uuid(req.dispatch_id),
        reason: req.reason,
    })
}
