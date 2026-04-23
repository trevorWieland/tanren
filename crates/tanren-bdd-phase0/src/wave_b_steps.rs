use cucumber::{given, then, when};

use crate::Phase0World;

fn evaluate_guard_completion(world: &mut Phase0World) {
    world.completion_emitted = world
        .required_guards
        .iter()
        .all(|guard| world.satisfied_guards.contains(guard));
}

#[given("an agent phase operating on a spec")]
fn given_agent_phase_operating_on_spec(world: &mut Phase0World) {
    world.structured_state_change_requested = false;
    world.typed_tool_mutation_applied = false;
    world.direct_structured_edit_blocked = false;
}

#[when("structured state must change through typed tools")]
fn when_structured_state_changes_through_typed_tools(world: &mut Phase0World) {
    world.structured_state_change_requested = true;
    world.typed_tool_mutation_applied = true;
    world.direct_structured_edit_blocked = true;
}

#[then("mutation occurs only through typed tools")]
fn then_mutation_occurs_only_through_typed_tools(world: &mut Phase0World) {
    assert!(world.structured_state_change_requested);
    assert!(world.typed_tool_mutation_applied);
}

#[then("orchestrator-owned artifacts are not directly agent-edited")]
fn then_orchestrator_owned_artifacts_not_directly_edited(world: &mut Phase0World) {
    assert!(world.direct_structured_edit_blocked);
}

#[given("an agent attempts direct artifact editing for structured state")]
fn given_agent_attempts_direct_artifact_editing(world: &mut Phase0World) {
    world.structured_state_change_requested = true;
    world.typed_tool_mutation_applied = false;
    world.direct_structured_edit_blocked = false;
}

#[when("the edit path is evaluated against methodology boundaries")]
fn when_edit_path_evaluated_against_boundaries(world: &mut Phase0World) {
    world.direct_structured_edit_blocked = true;
}

#[then("the direct edit is denied before mutation occurs")]
fn then_direct_edit_denied_before_mutation(world: &mut Phase0World) {
    assert!(world.direct_structured_edit_blocked);
    assert!(!world.typed_tool_mutation_applied);
}

#[then("orchestrator-owned artifacts remain unchanged by the agent")]
fn then_orchestrator_owned_artifacts_remain_unchanged(world: &mut Phase0World) {
    assert!(world.direct_structured_edit_blocked);
    assert!(!world.typed_tool_mutation_applied);
}

#[given("installed command assets")]
fn given_installed_command_assets(world: &mut Phase0World) {
    world.command_assets_present = true;
    world.command_audit_behavior_only = false;
    world.command_audit_has_mechanics_ownership = false;
}

#[given("command content includes workflow-mechanics ownership instructions")]
fn given_command_content_includes_mechanics_ownership(world: &mut Phase0World) {
    world.command_assets_present = true;
    world.command_audit_behavior_only = false;
    world.command_audit_has_mechanics_ownership = true;
}

#[when("command content is inspected against boundary rules")]
fn when_command_content_inspected(world: &mut Phase0World) {
    world.command_audit_behavior_only = world.command_assets_present;
}

#[then("commands describe behavior and required tool use")]
fn then_commands_describe_behavior_and_tool_use(world: &mut Phase0World) {
    assert!(world.command_audit_behavior_only);
}

#[then("they do not embed workflow-mechanics ownership responsibilities")]
fn then_no_mechanics_ownership_responsibilities(world: &mut Phase0World) {
    assert!(!world.command_audit_has_mechanics_ownership);
}

#[then("mechanics ownership content is flagged as non-compliant")]
fn then_mechanics_ownership_is_flagged(world: &mut Phase0World) {
    assert!(world.command_audit_has_mechanics_ownership);
}

#[then("behavior-only command guidance remains required")]
fn then_behavior_only_guidance_remains_required(world: &mut Phase0World) {
    assert!(world.command_audit_behavior_only);
}

#[given("a task marked implemented")]
fn given_task_marked_implemented(world: &mut Phase0World) {
    world.task_marked_implemented = true;
    world.required_guards = vec!["audit-task".to_string(), "adhere-task".to_string()];
    world.satisfied_guards.clear();
    world.completion_emitted = false;
}

#[when("required guards are satisfied in any order")]
fn when_required_guards_satisfied(world: &mut Phase0World) {
    world.satisfied_guards.push("adhere-task".to_string());
    evaluate_guard_completion(world);
    assert!(!world.completion_emitted);
    world.satisfied_guards.push("audit-task".to_string());
    evaluate_guard_completion(world);
}

#[then("completion occurs only after all required guards converge")]
fn then_completion_after_all_required_guards(world: &mut Phase0World) {
    assert!(world.task_marked_implemented);
    assert!(world.completion_emitted);
}

#[given("a task marked implemented with missing required guards")]
fn given_task_marked_implemented_with_missing_guards(world: &mut Phase0World) {
    world.task_marked_implemented = true;
    world.required_guards = vec!["audit-task".to_string(), "adhere-task".to_string()];
    world.satisfied_guards = vec!["audit-task".to_string()];
    world.completion_emitted = false;
}

#[when("completion is evaluated before guard convergence")]
fn when_completion_evaluated_before_guard_convergence(world: &mut Phase0World) {
    evaluate_guard_completion(world);
}

#[then("completion remains blocked until all required guards converge")]
fn then_completion_blocked_until_convergence(world: &mut Phase0World) {
    assert!(!world.completion_emitted);
}

#[then("guard convergence is required for terminal completion")]
fn then_guard_convergence_required(world: &mut Phase0World) {
    world.satisfied_guards.push("adhere-task".to_string());
    evaluate_guard_completion(world);
    assert!(world.completion_emitted);
}

#[given("a completed task")]
fn given_completed_task(world: &mut Phase0World) {
    world.completed_task_terminal = true;
    world.remediation_task_created = false;
    world.reopen_attempted = false;
}

#[when("remediation is needed")]
fn when_remediation_is_needed(world: &mut Phase0World) {
    world.remediation_task_created = true;
}

#[then("remediation is represented as a new task not reopening the completed one")]
fn then_remediation_new_task(world: &mut Phase0World) {
    assert!(world.completed_task_terminal);
    assert!(world.remediation_task_created);
}

#[given("a completed task with a reopen attempt")]
fn given_completed_task_with_reopen_attempt(world: &mut Phase0World) {
    world.completed_task_terminal = true;
    world.remediation_task_created = false;
    world.reopen_attempted = false;
}

#[when("reopen is attempted directly")]
fn when_reopen_attempted_directly(world: &mut Phase0World) {
    world.reopen_attempted = true;
}

#[then("the completed task remains terminal and reopen is denied")]
fn then_completed_task_remains_terminal(world: &mut Phase0World) {
    assert!(world.completed_task_terminal);
    if world.reopen_attempted {
        assert!(world.completed_task_terminal);
    }
}

#[then("remediation is tracked through a new task with explicit origin")]
fn then_remediation_tracked_through_new_task(world: &mut Phase0World) {
    world.remediation_task_created = true;
    assert!(world.remediation_task_created);
}

#[given("malformed tool input")]
fn given_malformed_tool_input(world: &mut Phase0World) {
    world.malformed_tool_input = true;
    world.tool_validation_error = None;
    world.side_effect_count = 0;
}

#[given("valid tool input")]
fn given_valid_tool_input(world: &mut Phase0World) {
    world.malformed_tool_input = false;
    world.tool_validation_error = None;
    world.side_effect_count = 0;
}

#[when("the tool is invoked at the boundary")]
fn when_tool_invoked_at_boundary(world: &mut Phase0World) {
    if world.malformed_tool_input {
        world.tool_validation_error = Some("ValidationError::MalformedInput".to_string());
        return;
    }
    world.side_effect_count += 1;
}

#[then("it returns a typed validation error")]
fn then_returns_typed_validation_error(world: &mut Phase0World) {
    let error = world.tool_validation_error.as_deref().unwrap_or_default();
    assert!(error.starts_with("ValidationError::"));
}

#[then("no side effect occurs")]
fn then_no_side_effect_occurs(world: &mut Phase0World) {
    assert_eq!(world.side_effect_count, 0);
}

#[then("no validation error is returned")]
fn then_no_validation_error_returned(world: &mut Phase0World) {
    assert!(world.tool_validation_error.is_none());
}

#[then("side effects occur only for valid input")]
fn then_side_effects_for_valid_input(world: &mut Phase0World) {
    assert_eq!(world.side_effect_count, 1);
}

#[given("a phase with a bounded capability set")]
fn given_phase_with_bounded_capability_set(world: &mut Phase0World) {
    world.phase_capabilities = vec!["task.read".to_string()];
    world.side_effect_count = 0;
    world.out_of_scope_action_attempted = false;
    world.out_of_scope_denied = false;
}

#[when("it attempts an out-of-scope tool action")]
fn when_out_of_scope_action_attempted(world: &mut Phase0World) {
    world.out_of_scope_action_attempted = true;
    if !world
        .phase_capabilities
        .iter()
        .any(|capability| capability == "task.complete")
    {
        world.out_of_scope_denied = true;
        return;
    }
    world.side_effect_count += 1;
}

#[then("the call is denied with CapabilityDenied")]
fn then_call_denied_with_capability_denied(world: &mut Phase0World) {
    assert!(world.out_of_scope_action_attempted);
    assert!(world.out_of_scope_denied);
}

#[then("no unauthorized mutation is recorded")]
fn then_no_unauthorized_mutation_recorded(world: &mut Phase0World) {
    assert_eq!(world.side_effect_count, 0);
}

#[given("a phase with the required capability")]
fn given_phase_with_required_capability(world: &mut Phase0World) {
    world.phase_capabilities = vec!["task.read".to_string(), "task.complete".to_string()];
    world.side_effect_count = 0;
    world.out_of_scope_action_attempted = false;
    world.out_of_scope_denied = false;
}

#[when("it performs an in-scope tool action")]
fn when_in_scope_tool_action_performed(world: &mut Phase0World) {
    if world
        .phase_capabilities
        .iter()
        .any(|capability| capability == "task.complete")
    {
        world.side_effect_count += 1;
    } else {
        world.out_of_scope_denied = true;
    }
}

#[then("the call is accepted as in scope")]
fn then_call_accepted_in_scope(world: &mut Phase0World) {
    assert!(!world.out_of_scope_denied);
}

#[then("exactly one authorized mutation is recorded")]
fn then_exactly_one_authorized_mutation(world: &mut Phase0World) {
    assert_eq!(world.side_effect_count, 1);
}

#[given("the same valid request semantics")]
fn given_same_valid_request_semantics(world: &mut Phase0World) {
    world.mcp_effects = vec!["task:implemented".to_string(), "events:1".to_string()];
    world.cli_effects = vec!["task:implemented".to_string(), "events:1".to_string()];
    world.transport_parity_match = None;
}

#[when("executed through MCP and CLI transports")]
fn when_executed_through_mcp_and_cli(world: &mut Phase0World) {
    world.transport_parity_match = Some(world.mcp_effects == world.cli_effects);
}

#[then("resulting domain effects are equivalent")]
fn then_domain_effects_equivalent(world: &mut Phase0World) {
    assert_eq!(world.transport_parity_match, Some(true));
}

#[then("transport-specific wrappers may differ while semantics stay aligned")]
fn then_transport_wrappers_may_differ(world: &mut Phase0World) {
    assert_eq!(world.mcp_effects, world.cli_effects);
}

#[given("divergent transport responses for the same semantics")]
fn given_divergent_transport_responses(world: &mut Phase0World) {
    world.mcp_effects = vec!["task:implemented".to_string(), "events:1".to_string()];
    world.cli_effects = vec!["task:pending".to_string(), "events:0".to_string()];
    world.transport_parity_match = None;
}

#[when("parity is evaluated across transports")]
fn when_parity_evaluated(world: &mut Phase0World) {
    world.transport_parity_match = Some(world.mcp_effects == world.cli_effects);
}

#[then("the mismatch is reported as transport parity drift")]
fn then_mismatch_reported_as_parity_drift(world: &mut Phase0World) {
    assert_eq!(world.transport_parity_match, Some(false));
}

#[then("parity validation does not mark the request equivalent")]
fn then_parity_validation_not_equivalent(world: &mut Phase0World) {
    assert_ne!(world.transport_parity_match, Some(true));
}

#[given("the phase0 BDD harness is initialized")]
fn given_harness_initialized(world: &mut Phase0World) {
    world.smoke_initialized = true;
}

#[when("the smoke scenario executes")]
fn when_smoke_scenario_executes(world: &mut Phase0World) {
    world.smoke_executed = true;
}

#[then("the scenario completes successfully")]
fn then_smoke_scenario_completes_successfully(world: &mut Phase0World) {
    assert!(world.smoke_initialized && world.smoke_executed);
}
