//! Single source-of-truth methodology tool registry.
//!
//! This module defines the full tool surface once and derives:
//! - MCP catalog entries (`name`, `description`, `input_schema`, `_meta`)
//! - JSON argument decode + typed service dispatch
//! - mutation classification for session enforcement

use std::borrow::Cow;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use rmcp::model::{JsonObject, Meta, Tool};
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use tanren_app_services::methodology::{
    CapabilityScope, MethodologyError, MethodologyService, PhaseId, ToolError,
};
use tanren_contract::methodology::{METHODOLOGY_SCHEMA_NAMESPACE, METHODOLOGY_SCHEMA_VERSION};

pub(crate) type DispatchFuture<'a> = Pin<Box<dyn Future<Output = CallResult> + Send + 'a>>;
pub(crate) type DispatchFn = for<'a> fn(
    &'a MethodologyService,
    &'a CapabilityScope,
    &'a PhaseId,
    Value,
) -> DispatchFuture<'a>;

pub(crate) type SchemaBuilder = fn() -> JsonObject;

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

#[derive(Clone, Copy)]
pub(crate) struct ToolRegistration {
    pub name: &'static str,
    pub description: &'static str,
    pub mutation: bool,
    pub schema_builder: SchemaBuilder,
    pub dispatch: DispatchFn,
}

macro_rules! define_tools {
    ($(
        {
            id: $id:ident,
            name: $name:literal,
            description: $description:literal,
            params: $params:ty,
            method: $method:ident,
            mutation: $mutation:expr
        }
    ),+ $(,)?) => {
        $(
            fn $id<'a>(
                service: &'a MethodologyService,
                scope: &'a CapabilityScope,
                phase: &'a PhaseId,
                args: Value,
            ) -> DispatchFuture<'a> {
                Box::pin(async move {
                    match decode::<$params>($name, args) {
                        Ok(params) => wrap(service.$method(scope, phase, params).await),
                        Err(err) => CallResult::Err(err),
                    }
                })
            }
        )+

        const REGISTRY: &[ToolRegistration] = &[
            $(
                ToolRegistration {
                    name: $name,
                    description: $description,
                    mutation: $mutation,
                    schema_builder: schema_object::<$params>,
                    dispatch: $id,
                },
            )+
        ];
    };
}

define_tools! {
    {
        id: dispatch_create_task,
        name: "create_task",
        description: "Create a new task in a spec. Emits TaskCreated and returns the new task_id.",
        params: tanren_contract::methodology::CreateTaskParams,
        method: create_task,
        mutation: true
    },
    {
        id: dispatch_start_task,
        name: "start_task",
        description: "Transition a task Pending → InProgress. Idempotent on InProgress.",
        params: tanren_contract::methodology::StartTaskParams,
        method: start_task,
        mutation: true
    },
    {
        id: dispatch_complete_task,
        name: "complete_task",
        description: "Transition a task InProgress → Implemented. Required guards still gate Complete.",
        params: tanren_contract::methodology::CompleteTaskParams,
        method: complete_task,
        mutation: true
    },
    {
        id: dispatch_mark_task_guard_satisfied,
        name: "mark_task_guard_satisfied",
        description: "Mark one completion guard satisfied; emits TaskCompleted when required guards converge.",
        params: tanren_contract::methodology::MarkTaskGuardSatisfiedParams,
        method: mark_task_guard_satisfied_with_params,
        mutation: true
    },
    {
        id: dispatch_revise_task,
        name: "revise_task",
        description: "Non-transitional revision of description or acceptance criteria.",
        params: tanren_contract::methodology::ReviseTaskParams,
        method: revise_task,
        mutation: true
    },
    {
        id: dispatch_abandon_task,
        name: "abandon_task",
        description: "Terminal abandonment with typed disposition and provenance.",
        params: tanren_contract::methodology::AbandonTaskParams,
        method: abandon_task,
        mutation: true
    },
    {
        id: dispatch_list_tasks,
        name: "list_tasks",
        description: "Projection: all tasks for a spec with current status.",
        params: tanren_contract::methodology::ListTasksParams,
        method: list_tasks,
        mutation: false
    },
    {
        id: dispatch_add_finding,
        name: "add_finding",
        description: "Record an audit / demo / investigation / feedback finding.",
        params: tanren_contract::methodology::AddFindingParams,
        method: add_finding,
        mutation: true
    },
    {
        id: dispatch_record_rubric_score,
        name: "record_rubric_score",
        description: "Record a per-pillar rubric score. Score<passing requires a fix_now finding.",
        params: tanren_contract::methodology::RecordRubricScoreParams,
        method: record_rubric_score,
        mutation: true
    },
    {
        id: dispatch_record_non_negotiable_compliance,
        name: "record_non_negotiable_compliance",
        description: "Record a pass/fail decision on a named non-negotiable.",
        params: tanren_contract::methodology::RecordNonNegotiableComplianceParams,
        method: record_non_negotiable_compliance,
        mutation: true
    },
    {
        id: dispatch_set_spec_title,
        name: "set_spec_title",
        description: "Set the spec's title (frontmatter).",
        params: tanren_contract::methodology::SetSpecTitleParams,
        method: set_spec_title,
        mutation: true
    },
    {
        id: dispatch_set_spec_non_negotiables,
        name: "set_spec_non_negotiables",
        description: "Full-replace the spec's non-negotiables list.",
        params: tanren_contract::methodology::SetSpecNonNegotiablesParams,
        method: set_spec_non_negotiables,
        mutation: true
    },
    {
        id: dispatch_add_spec_acceptance_criterion,
        name: "add_spec_acceptance_criterion",
        description: "Append one acceptance criterion to the spec frontmatter.",
        params: tanren_contract::methodology::AddSpecAcceptanceCriterionParams,
        method: add_spec_acceptance_criterion,
        mutation: true
    },
    {
        id: dispatch_set_spec_demo_environment,
        name: "set_spec_demo_environment",
        description: "Set the spec's demo-environment block.",
        params: tanren_contract::methodology::SetSpecDemoEnvironmentParams,
        method: set_spec_demo_environment,
        mutation: true
    },
    {
        id: dispatch_set_spec_dependencies,
        name: "set_spec_dependencies",
        description: "Set the spec's dependency graph (depends_on_spec_ids etc.).",
        params: tanren_contract::methodology::SetSpecDependenciesParams,
        method: set_spec_dependencies,
        mutation: true
    },
    {
        id: dispatch_set_spec_base_branch,
        name: "set_spec_base_branch",
        description: "Set the spec's base branch.",
        params: tanren_contract::methodology::SetSpecBaseBranchParams,
        method: set_spec_base_branch,
        mutation: true
    },
    {
        id: dispatch_set_spec_relevance_context,
        name: "set_spec_relevance_context",
        description: "Set the spec's relevance context (touched files/language/tags/category).",
        params: tanren_contract::methodology::SetSpecRelevanceContextParams,
        method: set_spec_relevance_context,
        mutation: true
    },
    {
        id: dispatch_add_demo_step,
        name: "add_demo_step",
        description: "Add a demo step with id, mode, description, and expected_observable.",
        params: tanren_contract::methodology::AddDemoStepParams,
        method: add_demo_step,
        mutation: true
    },
    {
        id: dispatch_mark_demo_step_skip,
        name: "mark_demo_step_skip",
        description: "Mark a demo step as skipped with a reason.",
        params: tanren_contract::methodology::MarkDemoStepSkipParams,
        method: mark_demo_step_skip,
        mutation: true
    },
    {
        id: dispatch_append_demo_result,
        name: "append_demo_result",
        description: "Append an observed result (status + observed) for a demo step.",
        params: tanren_contract::methodology::AppendDemoResultParams,
        method: append_demo_result,
        mutation: true
    },
    {
        id: dispatch_add_signpost,
        name: "add_signpost",
        description: "Record a signpost against a task or spec scope.",
        params: tanren_contract::methodology::AddSignpostParams,
        method: add_signpost,
        mutation: true
    },
    {
        id: dispatch_update_signpost_status,
        name: "update_signpost_status",
        description: "Update a signpost's status (and optional resolution text).",
        params: tanren_contract::methodology::UpdateSignpostStatusParams,
        method: update_signpost_status,
        mutation: true
    },
    {
        id: dispatch_report_phase_outcome,
        name: "report_phase_outcome",
        description: "End-of-phase outcome: complete | blocked | error.",
        params: tanren_contract::methodology::ReportPhaseOutcomeParams,
        method: report_phase_outcome,
        mutation: true
    },
    {
        id: dispatch_escalate_to_blocker,
        name: "escalate_to_blocker",
        description: "Escalate to a blocker phase. Capability-scoped to `investigate`.",
        params: tanren_contract::methodology::EscalateToBlockerParams,
        method: escalate_to_blocker,
        mutation: true
    },
    {
        id: dispatch_post_reply_directive,
        name: "post_reply_directive",
        description: "Record a feedback reply directive. Capability-scoped to `handle-feedback`.",
        params: tanren_contract::methodology::PostReplyDirectiveParams,
        method: post_reply_directive,
        mutation: true
    },
    {
        id: dispatch_create_issue,
        name: "create_issue",
        description: "Record a backlog issue. Returns a stable URN-shaped IssueRef until adapter reconciliation.",
        params: tanren_contract::methodology::CreateIssueParams,
        method: create_issue,
        mutation: true
    },
    {
        id: dispatch_list_relevant_standards,
        name: "list_relevant_standards",
        description: "Read-only: the baseline standards applicable to a spec.",
        params: tanren_contract::methodology::ListRelevantStandardsParams,
        method: list_relevant_standards_from_params,
        mutation: false
    },
    {
        id: dispatch_record_adherence_finding,
        name: "record_adherence_finding",
        description: "Record an adherence finding. Critical-importance standards cannot be deferred.",
        params: tanren_contract::methodology::RecordAdherenceFindingParams,
        method: record_adherence_finding,
        mutation: true
    }
}

#[must_use]
pub(crate) fn all() -> &'static [ToolRegistration] {
    REGISTRY
}

#[must_use]
pub(crate) fn find(name: &str) -> Option<&'static ToolRegistration> {
    REGISTRY.iter().find(|entry| entry.name == name)
}

#[must_use]
pub(crate) fn as_rmcp_tool(registration: &ToolRegistration) -> Tool {
    let mut tool = Tool::default();
    tool.name = Cow::Borrowed(registration.name);
    tool.description = Some(Cow::Borrowed(registration.description));
    tool.input_schema = Arc::new((registration.schema_builder)());
    tool.meta = Some(version_meta());
    tool
}

fn schema_object<T: JsonSchema>() -> JsonObject {
    let schema = schemars::schema_for!(T);
    let value = serde_json::to_value(schema).unwrap_or_else(|_| Value::Object(Map::new()));
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn version_meta() -> Meta {
    let mut m = Map::new();
    m.insert(
        "schema_namespace".into(),
        Value::String(METHODOLOGY_SCHEMA_NAMESPACE.into()),
    );
    m.insert(
        "schema_version".into(),
        Value::String(METHODOLOGY_SCHEMA_VERSION.into()),
    );
    Meta(m)
}

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
            Ok(value) => CallResult::Ok(value),
            Err(err) => CallResult::Err(ToolError::Internal {
                reason: format!("response serialize: {err}"),
            }),
        },
        Err(err) => CallResult::Err((&err).into()),
    }
}
