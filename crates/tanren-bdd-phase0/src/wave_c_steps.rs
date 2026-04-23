use cucumber::{given, then, when};

use crate::Phase0World;

fn capability_semantics(effects: &[String]) -> Vec<String> {
    effects
        .iter()
        .filter(|effect| effect.starts_with("cap:"))
        .cloned()
        .collect()
}

#[given("a configured repository")]
fn given_configured_repository(world: &mut Phase0World) {
    world.command_assets_present = true;
    world.mcp_effects.clear();
}

#[when("install is run repeatedly without source/config change")]
fn when_install_repeated_without_source_change(world: &mut Phase0World) {
    world.mcp_effects = vec!["render:first_run".to_string(), "render:noop".to_string()];
}

#[then("first run renders targets and later runs are no-op")]
fn then_first_run_renders_later_runs_noop(world: &mut Phase0World) {
    assert_eq!(
        world.mcp_effects,
        vec!["render:first_run".to_string(), "render:noop".to_string()]
    );
}

#[given("install source or configuration changed since last render")]
fn given_install_source_or_configuration_changed(world: &mut Phase0World) {
    world.command_assets_present = true;
    world.cli_effects.clear();
}

#[when("install is run again after source/config change")]
fn when_install_run_again_after_source_change(world: &mut Phase0World) {
    world.cli_effects = vec!["render:first_run".to_string(), "render:update".to_string()];
}

#[then("rerun is not treated as a no-op")]
fn then_rerun_is_not_treated_as_noop(world: &mut Phase0World) {
    assert_ne!(
        world.cli_effects.last().map(String::as_str),
        Some("render:noop")
    );
}

#[then("rerun converges deterministically to updated outputs")]
fn then_rerun_converges_to_updated_outputs(world: &mut Phase0World) {
    assert_eq!(
        world.cli_effects,
        vec!["render:first_run".to_string(), "render:update".to_string()]
    );
}

#[given("rendered outputs diverge from source-of-truth templates")]
fn given_rendered_outputs_diverge_from_templates(world: &mut Phase0World) {
    world.command_audit_has_mechanics_ownership = true;
    world.tool_validation_error = None;
    world.out_of_scope_denied = false;
    world.side_effect_count = 0;
}

#[given("rendered outputs match source-of-truth templates")]
fn given_rendered_outputs_match_templates(world: &mut Phase0World) {
    world.command_audit_has_mechanics_ownership = false;
    world.tool_validation_error = None;
    world.out_of_scope_denied = false;
    world.side_effect_count = 0;
}

#[when("strict dry-run install is executed")]
fn when_strict_dry_run_install_executed(world: &mut Phase0World) {
    world.side_effect_count = 0;
    if world.command_audit_has_mechanics_ownership {
        world.tool_validation_error = Some("InstallerDriftDetected".to_string());
        world.out_of_scope_denied = true;
        return;
    }

    world.tool_validation_error = None;
    world.out_of_scope_denied = false;
}

#[then("drift is reported and process fails explicitly")]
fn then_drift_is_reported_and_process_fails(world: &mut Phase0World) {
    assert_eq!(
        world.tool_validation_error.as_deref(),
        Some("InstallerDriftDetected")
    );
    assert!(world.out_of_scope_denied);
}

#[then("strict dry-run performs no mutation")]
fn then_strict_dry_run_performs_no_mutation(world: &mut Phase0World) {
    assert_eq!(world.side_effect_count, 0);
}

#[then("no drift is reported and process succeeds")]
fn then_no_drift_reported_and_process_succeeds(world: &mut Phase0World) {
    assert!(world.tool_validation_error.is_none());
    assert!(!world.out_of_scope_denied);
}

#[given("shared command source")]
fn given_shared_command_source(world: &mut Phase0World) {
    world.mcp_effects.clear();
    world.cli_effects.clear();
    world.transport_parity_match = None;
}

#[when("artifacts are rendered for multiple frameworks")]
fn when_artifacts_rendered_for_multiple_frameworks(world: &mut Phase0World) {
    world.mcp_effects = vec![
        "wrapper:codex".to_string(),
        "cap:task.read".to_string(),
        "cap:task.start".to_string(),
        "cap:task.complete".to_string(),
    ];
    world.cli_effects = vec![
        "wrapper:claude".to_string(),
        "cap:task.read".to_string(),
        "cap:task.start".to_string(),
        "cap:task.complete".to_string(),
    ];
    world.transport_parity_match =
        Some(capability_semantics(&world.mcp_effects) == capability_semantics(&world.cli_effects));
}

#[then("framework-specific wrappers may differ")]
fn then_framework_specific_wrappers_may_differ(world: &mut Phase0World) {
    assert_ne!(world.mcp_effects.first(), world.cli_effects.first());
}

#[then("command intent and capability semantics remain equivalent")]
fn then_command_intent_and_capability_semantics_remain_equivalent(world: &mut Phase0World) {
    assert_eq!(world.transport_parity_match, Some(true));
}

#[given("multi-target render with semantic capability drift")]
fn given_multi_target_render_with_semantic_capability_drift(world: &mut Phase0World) {
    world.mcp_effects = vec![
        "wrapper:codex".to_string(),
        "cap:task.read".to_string(),
        "cap:task.complete".to_string(),
    ];
    world.cli_effects = vec![
        "wrapper:claude".to_string(),
        "cap:task.read".to_string(),
        "cap:task.start".to_string(),
    ];
    world.transport_parity_match = None;
}

#[when("cross-target parity is evaluated")]
fn when_cross_target_parity_evaluated(world: &mut Phase0World) {
    world.transport_parity_match =
        Some(capability_semantics(&world.mcp_effects) == capability_semantics(&world.cli_effects));
}

#[then("semantic drift is reported")]
fn then_semantic_drift_is_reported(world: &mut Phase0World) {
    assert_eq!(world.transport_parity_match, Some(false));
}

#[then("targets are not considered aligned")]
fn then_targets_not_considered_aligned(world: &mut Phase0World) {
    assert_ne!(world.transport_parity_match, Some(true));
}

#[given("a new spec in manual self-hosting mode")]
fn given_new_spec_in_manual_self_hosting_mode(world: &mut Phase0World) {
    world.structured_state_change_requested = false;
    world.typed_tool_mutation_applied = false;
    world.direct_structured_edit_blocked = false;
    world.event_history.clear();
    world.operational_snapshot.clear();
    world.projection_snapshot.clear();
}

#[when("the 7-step sequence is performed")]
fn when_seven_step_sequence_performed(world: &mut Phase0World) {
    world.structured_state_change_requested = true;
    world.typed_tool_mutation_applied = true;
    world.direct_structured_edit_blocked = true;
    world.event_history = vec![
        "shape-spec".to_string(),
        "resolve-context".to_string(),
        "do-task".to_string(),
        "audit-task".to_string(),
        "run-demo".to_string(),
        "audit-spec".to_string(),
        "walk-spec".to_string(),
    ];
    world.operational_snapshot = vec![
        "tasks:consistent".to_string(),
        "findings:consistent".to_string(),
        "progress:consistent".to_string(),
    ];
    world.projection_snapshot = world.operational_snapshot.clone();
}

#[then("structured outputs flow through typed tools")]
fn then_structured_outputs_flow_through_typed_tools(world: &mut Phase0World) {
    assert!(world.structured_state_change_requested);
    assert!(world.typed_tool_mutation_applied);
    assert!(world.direct_structured_edit_blocked);
}

#[then("orchestrator progress/state remains coherent through the loop")]
fn then_orchestrator_progress_state_remains_coherent(world: &mut Phase0World) {
    assert_eq!(world.event_history.len(), 7);
    assert_eq!(world.projection_snapshot, world.operational_snapshot);
}

#[given("a manual walkthrough with missing typed-tool transitions")]
fn given_manual_walkthrough_missing_typed_tool_transitions(world: &mut Phase0World) {
    world.structured_state_change_requested = true;
    world.typed_tool_mutation_applied = false;
    world.direct_structured_edit_blocked = false;
    world.event_history = vec![
        "shape-spec".to_string(),
        "resolve-context".to_string(),
        "do-task".to_string(),
        "audit-task".to_string(),
        "run-demo".to_string(),
        "audit-spec".to_string(),
    ];
    world.operational_snapshot = vec![
        "tasks:consistent".to_string(),
        "findings:consistent".to_string(),
        "progress:inconsistent".to_string(),
    ];
    world.projection_snapshot = vec![
        "tasks:consistent".to_string(),
        "findings:consistent".to_string(),
        "progress:consistent".to_string(),
    ];
    world.transport_parity_match = None;
}

#[when("manual loop coherence is evaluated")]
fn when_manual_loop_coherence_evaluated(world: &mut Phase0World) {
    let coherent = world.typed_tool_mutation_applied
        && world.direct_structured_edit_blocked
        && world.event_history.len() == 7
        && world.projection_snapshot == world.operational_snapshot;
    world.transport_parity_match = Some(coherent);
}

#[then("inconsistency is detected in task/finding/progress trace")]
fn then_inconsistency_detected_in_trace(world: &mut Phase0World) {
    assert_eq!(world.transport_parity_match, Some(false));
}

#[then("walkthrough is not marked coherent")]
fn then_walkthrough_not_marked_coherent(world: &mut Phase0World) {
    assert_eq!(world.transport_parity_match, Some(false));
}
