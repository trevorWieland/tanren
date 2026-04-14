//! Conversion impls between contract request types and domain commands.
//!
//! Validation happens here: empty strings are rejected and timeouts are
//! bounds-checked. Enum fields are already typed in the contract, so no
//! string parsing is needed.

use tanren_domain::{
    ActorContext, ApiKeyId, AuthMode, ConfigEnv, CreateDispatch, NonEmptyString, OrgId, ProjectId,
    TeamId, TimeoutSecs, UserId,
};

use crate::error::ContractError;
use crate::request::CreateDispatchRequest;

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
            phase: req.phase,
            cli: req.cli,
            auth_mode: req.auth_mode.unwrap_or(AuthMode::ApiKey),
            branch,
            spec_folder,
            workflow_id,
            mode: req.mode,
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

/// Validate that a string is non-empty and wrap it in [`NonEmptyString`].
fn try_non_empty(field: &'static str, value: String) -> Result<NonEmptyString, ContractError> {
    NonEmptyString::try_new(value).map_err(|e| ContractError::InvalidField {
        field: field.to_owned(),
        reason: e.to_string(),
    })
}
