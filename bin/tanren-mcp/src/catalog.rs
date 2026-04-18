//! Tool catalog — the full `tanren.methodology.v1` surface published
//! over MCP.
//!
//! Each entry is a `rmcp::model::Tool` populated from the contract
//! crate's JSON Schema for the corresponding params type. `_meta`
//! carries `schema_version` so clients can gate on a minimum
//! acceptable contract version per
//! `agent-tool-surface.md §7 (versioning)`.

use std::borrow::Cow;
use std::sync::Arc;

use rmcp::model::{JsonObject, Meta, Tool};
use schemars::JsonSchema;
use serde_json::{Map, Value};
use tanren_contract::methodology::{METHODOLOGY_SCHEMA_NAMESPACE, METHODOLOGY_SCHEMA_VERSION};

/// Build the full tool catalog. Returned in a stable order: §3.1
/// through §3.8.
#[must_use]
pub(crate) fn all_tools() -> Vec<Tool> {
    let mut out = Vec::with_capacity(26);
    out.extend(task_tools());
    out.extend(finding_tools());
    out.extend(spec_tools());
    out.extend(demo_tools());
    out.extend(signpost_tools());
    out.extend(phase_tools());
    out.extend(issue_and_adherence_tools());
    out
}

fn task_tools() -> Vec<Tool> {
    use tanren_contract::methodology::{
        AbandonTaskParams, CompleteTaskParams, CreateTaskParams, ListTasksParams,
        MarkTaskGuardSatisfiedParams, ReviseTaskParams, StartTaskParams,
    };
    vec![
        tool_from::<CreateTaskParams>(
            "create_task",
            "Create a new task in a spec. Emits TaskCreated and returns the new task_id.",
        ),
        tool_from::<StartTaskParams>(
            "start_task",
            "Transition a task Pending → InProgress. Idempotent on InProgress.",
        ),
        tool_from::<CompleteTaskParams>(
            "complete_task",
            "Transition a task InProgress → Implemented. Required guards still gate Complete.",
        ),
        tool_from::<MarkTaskGuardSatisfiedParams>(
            "mark_task_guard_satisfied",
            "Mark one completion guard satisfied; emits TaskCompleted when required guards converge.",
        ),
        tool_from::<ReviseTaskParams>(
            "revise_task",
            "Non-transitional revision of description or acceptance criteria.",
        ),
        tool_from::<AbandonTaskParams>(
            "abandon_task",
            "Terminal abandonment. Requires non-empty replacements[] or explicit user-discard note.",
        ),
        tool_from::<ListTasksParams>(
            "list_tasks",
            "Projection: all tasks for a spec with current status.",
        ),
    ]
}

fn finding_tools() -> Vec<Tool> {
    use tanren_contract::methodology::{
        AddFindingParams, RecordNonNegotiableComplianceParams, RecordRubricScoreParams,
    };
    vec![
        tool_from::<AddFindingParams>(
            "add_finding",
            "Record an audit / demo / investigation / feedback finding.",
        ),
        tool_from::<RecordRubricScoreParams>(
            "record_rubric_score",
            "Record a per-pillar rubric score. Score<passing requires a fix_now finding.",
        ),
        tool_from::<RecordNonNegotiableComplianceParams>(
            "record_non_negotiable_compliance",
            "Record a pass/fail decision on a named non-negotiable.",
        ),
    ]
}

fn spec_tools() -> Vec<Tool> {
    use tanren_contract::methodology::{
        AddSpecAcceptanceCriterionParams, SetSpecBaseBranchParams, SetSpecDemoEnvironmentParams,
        SetSpecDependenciesParams, SetSpecNonNegotiablesParams, SetSpecTitleParams,
    };
    vec![
        tool_from::<SetSpecTitleParams>("set_spec_title", "Set the spec's title (frontmatter)."),
        tool_from::<SetSpecNonNegotiablesParams>(
            "set_spec_non_negotiables",
            "Full-replace the spec's non-negotiables list.",
        ),
        tool_from::<AddSpecAcceptanceCriterionParams>(
            "add_spec_acceptance_criterion",
            "Append one acceptance criterion to the spec frontmatter.",
        ),
        tool_from::<SetSpecDemoEnvironmentParams>(
            "set_spec_demo_environment",
            "Set the spec's demo-environment block.",
        ),
        tool_from::<SetSpecDependenciesParams>(
            "set_spec_dependencies",
            "Set the spec's dependency graph (depends_on_spec_ids etc.).",
        ),
        tool_from::<SetSpecBaseBranchParams>("set_spec_base_branch", "Set the spec's base branch."),
    ]
}

fn demo_tools() -> Vec<Tool> {
    use tanren_contract::methodology::{
        AddDemoStepParams, AppendDemoResultParams, MarkDemoStepSkipParams,
    };
    vec![
        tool_from::<AddDemoStepParams>(
            "add_demo_step",
            "Add a demo step with id, mode, description, and expected_observable.",
        ),
        tool_from::<MarkDemoStepSkipParams>(
            "mark_demo_step_skip",
            "Mark a demo step as skipped with a reason.",
        ),
        tool_from::<AppendDemoResultParams>(
            "append_demo_result",
            "Append an observed result (status + observed) for a demo step.",
        ),
    ]
}

fn signpost_tools() -> Vec<Tool> {
    use tanren_contract::methodology::{AddSignpostParams, UpdateSignpostStatusParams};
    vec![
        tool_from::<AddSignpostParams>(
            "add_signpost",
            "Record a signpost against a task or spec scope.",
        ),
        tool_from::<UpdateSignpostStatusParams>(
            "update_signpost_status",
            "Update a signpost's status (and optional resolution text).",
        ),
    ]
}

fn phase_tools() -> Vec<Tool> {
    use tanren_contract::methodology::{
        EscalateToBlockerParams, PostReplyDirectiveParams, ReportPhaseOutcomeParams,
    };
    vec![
        tool_from::<ReportPhaseOutcomeParams>(
            "report_phase_outcome",
            "End-of-phase outcome: complete | blocked | error.",
        ),
        tool_from::<EscalateToBlockerParams>(
            "escalate_to_blocker",
            "Escalate to a blocker phase. Capability-scoped to `investigate`.",
        ),
        tool_from::<PostReplyDirectiveParams>(
            "post_reply_directive",
            "Record a feedback reply directive. Capability-scoped to `handle-feedback`.",
        ),
    ]
}

fn issue_and_adherence_tools() -> Vec<Tool> {
    use tanren_contract::methodology::{
        CreateIssueParams, ListRelevantStandardsParams, RecordAdherenceFindingParams,
    };
    vec![
        tool_from::<CreateIssueParams>(
            "create_issue",
            "Record a backlog issue. Returns a stable URN-shaped IssueRef until adapter reconciliation.",
        ),
        tool_from::<ListRelevantStandardsParams>(
            "list_relevant_standards",
            "Read-only: the baseline standards applicable to a spec.",
        ),
        tool_from::<RecordAdherenceFindingParams>(
            "record_adherence_finding",
            "Record an adherence finding. Critical-importance standards cannot be deferred.",
        ),
    ]
}

/// Build a `rmcp::Tool` from a `JsonSchema`-deriving params type.
fn tool_from<T: JsonSchema>(name: &'static str, description: &'static str) -> Tool {
    let input_schema = schema_object::<T>();
    let mut tool = Tool::default();
    tool.name = Cow::Borrowed(name);
    tool.description = Some(Cow::Borrowed(description));
    tool.input_schema = Arc::new(input_schema);
    tool.meta = Some(version_meta());
    tool
}

/// Serialize a params type's JSON Schema into a `JsonObject` suitable
/// for `rmcp::Tool::input_schema`.
fn schema_object<T: JsonSchema>() -> JsonObject {
    let schema = schemars::schema_for!(T);
    let value = serde_json::to_value(schema).unwrap_or_else(|_| Value::Object(Map::new()));
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// Build the `_meta` block advertising the methodology schema
/// namespace + version.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_all_27_tools() {
        let tools = all_tools();
        // §3.1(7) + §3.2(3) + §3.3(6) + §3.4(3) + §3.5(2) + §3.6(3)
        // + §3.7(1) + §3.8(2) = 27 tool surface entries.
        // (Ingest/replay are §6 CLI-only transports, not registered
        // here.)
        assert_eq!(tools.len(), 27, "expected 27 methodology tools");
    }

    #[test]
    fn every_tool_has_schema_and_meta() {
        for t in all_tools() {
            assert!(!t.input_schema.is_empty(), "{} missing schema", t.name);
            assert!(t.meta.is_some(), "{} missing meta", t.name);
            assert!(t.description.is_some(), "{} missing description", t.name);
        }
    }

    #[test]
    fn tool_names_are_unique_and_snake_case() {
        let tools = all_tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate tool name in catalog");
        for t in all_tools() {
            for c in t.name.chars() {
                assert!(
                    c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit(),
                    "{} not snake_case",
                    t.name
                );
            }
        }
    }

    #[test]
    fn version_meta_advertises_v1() {
        let tool = all_tools().into_iter().next().expect("at least one tool");
        let meta = tool.meta.expect("meta");
        let ns = meta
            .0
            .get("schema_namespace")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let ver = meta
            .0
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(ns, "tanren.methodology.v1");
        assert_eq!(ver, "1.0.0");
    }
}
