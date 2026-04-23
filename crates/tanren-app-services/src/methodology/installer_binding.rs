//! Phase-specific binding prose for rendered commands.

use super::config::InstallBinding;
use super::source::CommandSource;
use tanren_domain::methodology::descriptor_by_name;

pub(super) fn binding_instructions(binding: InstallBinding, command: &CommandSource) -> String {
    match binding {
        InstallBinding::Mcp => build_phase_binding_instructions(
            command,
            "Use Tanren MCP tools for all structured mutations in this phase.",
            "MCP-first",
        ),
        InstallBinding::Cli => build_phase_binding_instructions(
            command,
            "Use the Tanren CLI for all structured mutations in this phase.",
            "CLI-first",
        ),
        InstallBinding::None => {
            "No tool binding is configured for this target; do not perform structured mutations from this command.".to_owned()
        }
    }
}

fn build_phase_binding_instructions(
    command: &CommandSource,
    leading: &str,
    mode_label: &str,
) -> String {
    let phase = command.frontmatter.name.as_str();
    let tools = command.frontmatter.declared_tools.as_slice();
    if tools.is_empty() {
        return format!(
            "{leading}\nNo declared tools for this command.\nCLI shape reference:\n`tanren-cli methodology --phase {phase} --spec-id <spec_uuid> --spec-folder <spec_dir> <noun> <verb> --json '<payload.json>'`."
        );
    }
    let mut lines = Vec::new();
    lines.push(leading.to_owned());
    lines.push(format!(
        "{mode_label} canonical invocation set for phase `{phase}`:"
    ));
    for tool in tools {
        let descriptor = descriptor_by_name(tool.as_str());
        let cli_cmd = descriptor.map_or_else(
            || "<unknown noun> <unknown verb>".to_owned(),
            |d| format!("{} {}", d.cli_noun, d.cli_verb),
        );
        let payload = minimal_valid_payload_json(tool).unwrap_or(r#"{"schema_version":"1.0.0"}"#);
        lines.push(format!("- MCP `{tool}` payload: `{payload}`"));
        lines.push(format!(
            "- CLI `{tool}` fallback: `tanren-cli methodology --phase {phase} --spec-id <spec_uuid> --spec-folder <spec_dir> {cli_cmd} --json '{payload}'`"
        ));
    }
    lines.join("\n")
}

fn minimal_valid_payload_json(tool: &str) -> Option<&'static str> {
    minimal_task_and_spec_payload_json(tool).or_else(|| minimal_demo_and_phase_payload_json(tool))
}

fn minimal_task_and_spec_payload_json(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "create_task" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"task title","description":"task description","origin":{"kind":"user"},"acceptance_criteria":[]}"#
        }
        "start_task" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000"}"#
        }
        "complete_task" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","evidence_refs":[]}"#
        }
        "reset_task_guards" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","reason":"retry from investigate loop"}"#
        }
        "revise_task" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","revised_description":"updated details","revised_acceptance":[],"reason":"clarify acceptance"}"#
        }
        "abandon_task" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","reason":"superseded","disposition":"replacement","replacements":[]}"#
        }
        "list_tasks" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}"#
        }
        "add_finding" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}"#
        }
        "record_rubric_score" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","pillar":"security","score":8,"target":10,"passing":7,"rationale":"needs additional hardening","supporting_finding_ids":["00000000-0000-0000-0000-000000000000"]}"#
        }
        "record_non_negotiable_compliance" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","name":"fail-closed-mcp","status":"pass","rationale":"envelope verification is enforced"}"#
        }
        "set_spec_title" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"Spec title"}"#
        }
        "set_spec_problem_statement" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","problem_statement":"Problem statement"}"#
        }
        "set_spec_motivations" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","motivations":["motivation"]}"#
        }
        "set_spec_expectations" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","expectations":["expectation"]}"#
        }
        "set_spec_planned_behaviors" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","planned_behaviors":["behavior"]}"#
        }
        "set_spec_implementation_plan" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","implementation_plan":["step 1"]}"#
        }
        "set_spec_non_negotiables" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","items":["non-negotiable"]}"#
        }
        "add_spec_acceptance_criterion" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","criterion":{"id":"ac-1","description":"criterion","measurable":"observable evidence"}}"#
        }
        "set_spec_demo_environment" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","demo_environment":{"connections":[{"name":"api","kind":"http","probe":"GET /healthz"}]}}"#
        }
        "set_spec_dependencies" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","dependencies":{"depends_on_spec_ids":[],"external_issue_refs":[]}}"#
        }
        "set_spec_base_branch" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","branch":"main"}"#
        }
        "set_spec_relevance_context" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","relevance_context":{"touched_files":["src/lib.rs"],"project_language":"rust","tags":["safety"],"category":"backend"}}"#
        }
        _ => return None,
    })
}

fn minimal_demo_and_phase_payload_json(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "add_demo_step" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","id":"step-1","mode":"RUN","description":"Run smoke flow","expected_observable":"No errors"}"#
        }
        "mark_demo_step_skip" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","step_id":"step-1","reason":"not applicable"}"#
        }
        "append_demo_result" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","step_id":"step-1","status":"pass","observed":"all checks green"}"#
        }
        "add_signpost" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"unresolved","problem":"problem statement","evidence":"evidence summary","tried":[],"files_affected":[]}"#
        }
        "update_signpost_status" => {
            r#"{"schema_version":"1.0.0","signpost_id":"00000000-0000-0000-0000-000000000000","status":"resolved","resolution":"resolved with deterministic projection"}"#
        }
        "report_phase_outcome" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}"#
        }
        "escalate_to_blocker" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","reason":"needs decision","options":["approve option A","approve option B"]}"#
        }
        "post_reply_directive" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","thread_ref":"github:org/repo#123","body":"Thanks for the feedback.","disposition":"ack"}"#
        }
        "create_issue" => {
            r#"{"schema_version":"1.0.0","origin_spec_id":"00000000-0000-0000-0000-000000000000","title":"Follow-up","description":"Track deferred work","suggested_spec_scope":"future-spec","priority":"medium"}"#
        }
        "list_relevant_standards" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}"#
        }
        "record_adherence_finding" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}"#
        }
        _ => return None,
    })
}
