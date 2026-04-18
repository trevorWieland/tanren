//! Tool-call dispatch: deserialize `request.arguments` into the
//! matching contract params type, invoke the `MethodologyService`
//! method, serialize the typed response (or typed `ToolError`).

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use tanren_app_services::methodology::{
    CapabilityScope, MethodologyError, MethodologyService, ToolError,
};

/// Result of one tool call: either a JSON response value, or a typed
/// `ToolError` to surface as `is_error = true` in the MCP envelope.
pub(crate) enum CallResult {
    Ok(Value),
    Err(ToolError),
}

impl CallResult {
    pub(crate) fn to_json(&self) -> String {
        match self {
            Self::Ok(v) => serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".into()),
            Self::Err(e) => serde_json::to_string_pretty(e).unwrap_or_else(|_| "{}".into()),
        }
    }

    pub(crate) const fn is_error(&self) -> bool {
        matches!(self, Self::Err(_))
    }
}

/// Dispatch one tool call. Unknown tool names surface as
/// `ToolError::NotFound` so the client sees a typed envelope.
pub(crate) async fn dispatch(
    service: &MethodologyService,
    scope: &CapabilityScope,
    phase: &str,
    tool: &str,
    args: Value,
) -> CallResult {
    use tanren_contract::methodology as c;

    macro_rules! call {
        ($params:ty, $method:ident) => {{
            match decode::<$params>(tool, args) {
                Ok(params) => wrap(service.$method(scope, phase, params).await),
                Err(e) => CallResult::Err(e),
            }
        }};
    }

    match tool {
        "create_task" => call!(c::CreateTaskParams, create_task),
        "start_task" => call!(c::StartTaskParams, start_task),
        "complete_task" => call!(c::CompleteTaskParams, complete_task),
        "revise_task" => call!(c::ReviseTaskParams, revise_task),
        "abandon_task" => call!(c::AbandonTaskParams, abandon_task),
        "list_tasks" => call!(c::ListTasksParams, list_tasks),
        "add_finding" => call!(c::AddFindingParams, add_finding),
        "record_rubric_score" => call!(c::RecordRubricScoreParams, record_rubric_score),
        "record_non_negotiable_compliance" => call!(
            c::RecordNonNegotiableComplianceParams,
            record_non_negotiable_compliance
        ),
        "set_spec_title" => call!(c::SetSpecTitleParams, set_spec_title),
        "set_spec_non_negotiables" => {
            call!(c::SetSpecNonNegotiablesParams, set_spec_non_negotiables)
        }
        "add_spec_acceptance_criterion" => call!(
            c::AddSpecAcceptanceCriterionParams,
            add_spec_acceptance_criterion
        ),
        "set_spec_demo_environment" => {
            call!(c::SetSpecDemoEnvironmentParams, set_spec_demo_environment)
        }
        "set_spec_dependencies" => call!(c::SetSpecDependenciesParams, set_spec_dependencies),
        "set_spec_base_branch" => call!(c::SetSpecBaseBranchParams, set_spec_base_branch),
        "add_demo_step" => call!(c::AddDemoStepParams, add_demo_step),
        "mark_demo_step_skip" => call!(c::MarkDemoStepSkipParams, mark_demo_step_skip),
        "append_demo_result" => call!(c::AppendDemoResultParams, append_demo_result),
        "add_signpost" => call!(c::AddSignpostParams, add_signpost),
        "update_signpost_status" => {
            call!(c::UpdateSignpostStatusParams, update_signpost_status)
        }
        "report_phase_outcome" => call!(c::ReportPhaseOutcomeParams, report_phase_outcome),
        "escalate_to_blocker" => call!(c::EscalateToBlockerParams, escalate_to_blocker),
        "post_reply_directive" => call!(c::PostReplyDirectiveParams, post_reply_directive),
        "create_issue" => call!(c::CreateIssueParams, create_issue),
        "list_relevant_standards" => {
            match decode::<c::ListRelevantStandardsParams>(tool, args) {
                Ok(params) => {
                    // Read-only; not async in the service surface.
                    wrap(service.list_relevant_standards(scope, phase, params.spec_id))
                }
                Err(e) => CallResult::Err(e),
            }
        }
        "record_adherence_finding" => {
            call!(c::RecordAdherenceFindingParams, record_adherence_finding)
        }
        other => CallResult::Err(ToolError::NotFound {
            resource: "tool".into(),
            key: other.to_owned(),
        }),
    }
}

/// Decode the raw JSON `args` into a concrete contract params type.
/// On failure, emit a typed `ToolError::ValidationFailed` with the
/// serde error's line/column in the field path so the client can
/// fix their payload.
fn decode<T: DeserializeOwned>(tool: &str, args: Value) -> Result<T, ToolError> {
    serde_json::from_value::<T>(args).map_err(|e| ToolError::ValidationFailed {
        field_path: format!("/arguments (line {} col {})", e.line(), e.column()),
        expected: format!("{tool} input per tanren.methodology.v1 schema"),
        actual: e.to_string(),
        remediation: "ensure arguments match the tool's input_schema".into(),
    })
}

fn wrap<R: Serialize>(result: Result<R, MethodologyError>) -> CallResult {
    match result {
        Ok(response) => match serde_json::to_value(response) {
            Ok(v) => CallResult::Ok(v),
            Err(e) => CallResult::Err(ToolError::Internal {
                reason: format!("response serialize: {e}"),
            }),
        },
        Err(err) => CallResult::Err((&err).into()),
    }
}
