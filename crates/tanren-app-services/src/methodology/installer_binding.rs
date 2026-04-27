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
    lines.push(
        "The orchestrator exports `TANREN_CLI`, `TANREN_DATABASE_URL`, `TANREN_CONFIG`, and `TANREN_SPEC_FOLDER`; use those values directly for CLI tool calls.".to_owned(),
    );
    for tool in tools {
        let descriptor = descriptor_by_name(tool.as_str());
        let cli_cmd = descriptor.map_or_else(
            || "<unknown noun> <unknown verb>".to_owned(),
            |d| format!("{} {}", d.cli_noun, d.cli_verb),
        );
        let payload = minimal_valid_payload_json(tool, phase)
            .unwrap_or_else(|| r#"{"schema_version":"1.0.0"}"#.to_owned());
        lines.push(format!("- MCP `{tool}` payload: `{payload}`"));
        lines.push(format!(
            "- CLI `{tool}` command: `\"$TANREN_CLI\" --database-url \"$TANREN_DATABASE_URL\" methodology --methodology-config \"$TANREN_CONFIG\" --phase {phase} --spec-id <spec_uuid> --spec-folder \"$TANREN_SPEC_FOLDER\" {cli_cmd} --json '{payload}'`"
        ));
    }
    lines.join("\n")
}

fn minimal_valid_payload_json(tool: &str, phase: &str) -> Option<String> {
    minimal_task_and_spec_payload_json(tool, phase)
        .or_else(|| minimal_lifecycle_payload_json(tool, phase))
        .or_else(|| minimal_demo_and_phase_payload_json(tool, phase))
}

fn minimal_task_and_spec_payload_json(tool: &str, phase: &str) -> Option<String> {
    Some(match tool {
        "create_task" => {
            if phase == "investigate" {
                r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"spec investigation follow-up","description":"task description","origin":{"kind":"spec_investigation","source_phase":"audit-spec","source_finding":"00000000-0000-0000-0000-000000000001"},"acceptance_criteria":[]}"#.to_owned()
            } else {
                r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"task title","description":"task description","origin":{"kind":"user"},"acceptance_criteria":[]}"#.to_owned()
            }
        }
        "start_task" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000"}"#.to_owned()
        }
        "complete_task" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","evidence_refs":[]}"#.to_owned()
        }
        "reset_task_guards" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","reason":"retry from investigate loop"}"#.to_owned()
        }
        "revise_task" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","revised_description":"updated details","revised_acceptance":[],"reason":"clarify acceptance"}"#.to_owned()
        }
        "abandon_task" => {
            r#"{"schema_version":"1.0.0","task_id":"00000000-0000-0000-0000-000000000000","reason":"superseded","disposition":"replacement","replacements":[]}"#.to_owned()
        }
        "list_tasks" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000"}"#.to_owned()
        }
        "add_finding" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","severity":"fix_now","title":"finding title","description":"finding details","source":{"kind":"audit","phase":"audit-spec","pillar":"security"}}"#.to_owned()
        }
        "list_findings" => {
            list_findings_payload(phase)
        }
        "record_rubric_score" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","pillar":"security","score":8,"target":10,"passing":7,"rationale":"needs additional hardening","supporting_finding_ids":["00000000-0000-0000-0000-000000000000"]}"#.to_owned()
        }
        "record_non_negotiable_compliance" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","scope":"spec","name":"fail-closed-mcp","status":"pass","rationale":"envelope verification is enforced"}"#.to_owned()
        }
        "set_spec_title" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","title":"Spec title"}"#.to_owned()
        }
        "set_spec_problem_statement" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","problem_statement":"Problem statement"}"#.to_owned()
        }
        "set_spec_motivations" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","motivations":["motivation"]}"#.to_owned()
        }
        "set_spec_expectations" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","expectations":["expectation"]}"#.to_owned()
        }
        "set_spec_planned_behaviors" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","planned_behaviors":["behavior"]}"#.to_owned()
        }
        "set_spec_implementation_plan" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","implementation_plan":["step 1"]}"#.to_owned()
        }
        "set_spec_non_negotiables" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","items":["non-negotiable"]}"#.to_owned()
        }
        "add_spec_acceptance_criterion" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","criterion":{"id":"ac-1","description":"criterion","measurable":"observable evidence"}}"#.to_owned()
        }
        "set_spec_demo_environment" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","demo_environment":{"connections":[{"name":"api","kind":"http","probe":"GET /healthz"}]}}"#.to_owned()
        }
        "set_spec_dependencies" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","dependencies":{"depends_on_spec_ids":[],"external_issue_refs":[]}}"#.to_owned()
        }
        "set_spec_base_branch" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","branch":"main"}"#.to_owned()
        }
        "set_spec_relevance_context" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","relevance_context":{"touched_files":["src/lib.rs"],"project_language":"rust","tags":["safety"],"category":"backend"}}"#.to_owned()
        }
        _ => return None,
    })
}

fn minimal_lifecycle_payload_json(tool: &str, phase: &str) -> Option<String> {
    let check_kind = check_kind_for_phase(phase);
    Some(match tool {
        "resolve_finding" | "reopen_finding" | "defer_finding" | "record_finding_still_open" => {
            format!(r#"{{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","evidence":{{"summary":"verified lifecycle state","evidence_refs":["check.log"],"check_kind":{{"kind":"{check_kind}"}}}}}}"#)
        }
        "supersede_finding" => {
            format!(r#"{{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","finding_id":"00000000-0000-0000-0000-000000000000","superseded_by":["00000000-0000-0000-0000-000000000001"],"evidence":{{"summary":"replacement finding captures the work","evidence_refs":["check.log"],"check_kind":{{"kind":"{check_kind}"}}}}}}"#)
        }
        "start_check_run" => {
            format!(r#"{{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","kind":{{"kind":"{check_kind}"}},"scope":{},"fingerprint":"{check_kind}:{phase}"}}"#, check_scope_json(phase))
        }
        "record_check_result" => {
            format!(r#"{{"schema_version":"1.0.0","check_run_id":"00000000-0000-0000-0000-000000000000","spec_id":"00000000-0000-0000-0000-000000000000","kind":{{"kind":"{check_kind}"}},"scope":{},"status":"pass","summary":"check passed","evidence_refs":["check.log"],"finding_ids":[]}}"#, check_scope_json(phase))
        }
        "record_check_failure" => {
            format!(r#"{{"schema_version":"1.0.0","check_run_id":"00000000-0000-0000-0000-000000000000","spec_id":"00000000-0000-0000-0000-000000000000","kind":{{"kind":"{check_kind}"}},"scope":{},"status":"fail","summary":"check failed","evidence_refs":["check.log"],"finding_ids":["00000000-0000-0000-0000-000000000000"]}}"#, check_scope_json(phase))
        }
        "record_investigation_attempt" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","fingerprint":"audit:spec","loop_index":1,"source_check":{"phase":"audit-spec","kind":{"kind":"audit"},"scope":{"scope":"spec"}},"source_findings":["00000000-0000-0000-0000-000000000000"],"evidence_refs":["investigation-report.json"],"root_causes":[{"description":"root cause summary","confidence":"high","category":"code_bug","affected_files":["src/lib.rs"]}]}"#.to_owned()
        }
        "list_investigation_attempts" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","fingerprint":"audit:spec"}"#.to_owned()
        }
        "link_root_cause_to_finding" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","attempt_id":"00000000-0000-0000-0000-000000000000","root_cause_id":"00000000-0000-0000-0000-000000000001","finding_id":"00000000-0000-0000-0000-000000000002","source_check":{"phase":"audit-spec","kind":{"kind":"audit"},"scope":{"scope":"spec"}}}"#.to_owned()
        }
        _ => return None,
    })
}

fn minimal_demo_and_phase_payload_json(tool: &str, phase: &str) -> Option<String> {
    Some(match tool {
        "add_demo_step" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","id":"step-1","mode":"RUN","description":"Run smoke flow","expected_observable":"No errors"}"#.to_owned()
        }
        "mark_demo_step_skip" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","step_id":"step-1","reason":"not applicable"}"#.to_owned()
        }
        "append_demo_result" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","step_id":"step-1","status":"pass","observed":"all checks green"}"#.to_owned()
        }
        "add_signpost" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"unresolved","problem":"problem statement","evidence":"evidence summary","tried":[],"files_affected":[]}"#.to_owned()
        }
        "update_signpost_status" => {
            r#"{"schema_version":"1.0.0","signpost_id":"00000000-0000-0000-0000-000000000000","status":"resolved","resolution":"resolved with deterministic projection"}"#.to_owned()
        }
        "report_phase_outcome" => {
            if matches!(phase, "do-task" | "audit-task" | "adhere-task") {
                r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","task_id":"00000000-0000-0000-0000-000000000001","outcome":{"outcome":"complete","summary":"phase complete"}}"#.to_owned()
            } else {
                r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","outcome":{"outcome":"complete","summary":"phase complete"}}"#.to_owned()
            }
        }
        "escalate_to_blocker" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","reason":"needs decision","options":["approve option A","approve option B"]}"#.to_owned()
        }
        "post_reply_directive" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","thread_ref":"github:org/repo#123","body":"Thanks for the feedback.","disposition":"ack"}"#.to_owned()
        }
        "create_issue" => {
            r#"{"schema_version":"1.0.0","origin_spec_id":"00000000-0000-0000-0000-000000000000","title":"Follow-up","description":"Track deferred work","suggested_spec_scope":"future-spec","priority":"medium"}"#.to_owned()
        }
        "list_relevant_standards" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","touched_files":[],"domains":[],"tags":[]}"#.to_owned()
        }
        "record_adherence_finding" => {
            r#"{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","standard":{"name":"input-validation","category":"security"},"severity":"fix_now","rationale":"missing validation on untrusted input"}"#.to_owned()
        }
        _ => return None,
    })
}

fn check_kind_for_phase(phase: &str) -> &'static str {
    if phase.starts_with("adhere") {
        "adherence"
    } else if phase == "run-demo" {
        "demo"
    } else if phase == "spec-gate" {
        "spec_gate"
    } else {
        "audit"
    }
}

fn check_scope_json(phase: &str) -> &'static str {
    if phase.ends_with("-task") {
        r#"{"scope":"task","task_id":"00000000-0000-0000-0000-000000000000"}"#
    } else {
        r#"{"scope":"spec"}"#
    }
}

fn list_findings_payload(phase: &str) -> String {
    let check_kind = check_kind_for_phase(phase);
    if phase.ends_with("-task") {
        format!(
            r#"{{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"task","task_id":"00000000-0000-0000-0000-000000000000","check_kind":{{"kind":"{check_kind}"}}}}"#
        )
    } else {
        format!(
            r#"{{"schema_version":"1.0.0","spec_id":"00000000-0000-0000-0000-000000000000","status":"open","severity":"fix_now","scope":"spec","check_kind":{{"kind":"{check_kind}"}}}}"#
        )
    }
}
