use std::env;

use cucumber::{World as _, given, then, when};

mod wave_auth_steps;
mod wave_b_steps;
mod wave_c_steps;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleState {
    New,
    Planned,
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MutationErrorCode {
    InvalidTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthErrorCode {
    InvalidIdentity,
    ReplayCredential,
}

#[derive(Debug, cucumber::World)]
struct Phase0World {
    smoke_initialized: bool,
    smoke_executed: bool,

    lifecycle_state: LifecycleState,
    lifecycle_trace: Vec<LifecycleState>,
    query_surface_status: LifecycleState,
    last_mutation_error: Option<MutationErrorCode>,
    persisted_revision: usize,
    persisted_revision_before_attempt: usize,

    successful_mutation_pending: bool,
    event_history: Vec<String>,
    projection_snapshot: Vec<String>,
    operational_snapshot: Vec<String>,
    replay_source_history: Vec<String>,
    replayed_snapshot: Vec<String>,
    replay_error: Option<String>,
    replay_applied_count: usize,

    actor_identity_valid: bool,
    replay_credential_fresh: bool,
    command_outcomes: Vec<String>,
    auth_errors: Vec<AuthErrorCode>,
    protected_mutation_count: usize,

    structured_state_change_requested: bool,
    typed_tool_mutation_applied: bool,
    direct_structured_edit_blocked: bool,
    command_assets_present: bool,
    command_audit_behavior_only: bool,
    command_audit_has_mechanics_ownership: bool,

    task_marked_implemented: bool,
    required_guards: Vec<String>,
    satisfied_guards: Vec<String>,
    completion_emitted: bool,
    completed_task_terminal: bool,
    remediation_task_created: bool,
    reopen_attempted: bool,

    malformed_tool_input: bool,
    tool_validation_error: Option<String>,
    side_effect_count: usize,
    phase_capabilities: Vec<String>,
    out_of_scope_action_attempted: bool,
    out_of_scope_denied: bool,
    mcp_effects: Vec<String>,
    cli_effects: Vec<String>,
    transport_parity_match: Option<bool>,
}

impl Default for Phase0World {
    fn default() -> Self {
        Self {
            smoke_initialized: false,
            smoke_executed: false,
            lifecycle_state: LifecycleState::New,
            lifecycle_trace: Vec::new(),
            query_surface_status: LifecycleState::New,
            last_mutation_error: None,
            persisted_revision: 0,
            persisted_revision_before_attempt: 0,
            successful_mutation_pending: false,
            event_history: Vec::new(),
            projection_snapshot: Vec::new(),
            operational_snapshot: Vec::new(),
            replay_source_history: Vec::new(),
            replayed_snapshot: Vec::new(),
            replay_error: None,
            replay_applied_count: 0,
            actor_identity_valid: false,
            replay_credential_fresh: false,
            command_outcomes: Vec::new(),
            auth_errors: Vec::new(),
            protected_mutation_count: 0,
            structured_state_change_requested: false,
            typed_tool_mutation_applied: false,
            direct_structured_edit_blocked: false,
            command_assets_present: false,
            command_audit_behavior_only: false,
            command_audit_has_mechanics_ownership: false,
            task_marked_implemented: false,
            required_guards: Vec::new(),
            satisfied_guards: Vec::new(),
            completion_emitted: false,
            completed_task_terminal: false,
            remediation_task_created: false,
            reopen_attempted: false,
            malformed_tool_input: false,
            tool_validation_error: None,
            side_effect_count: 0,
            phase_capabilities: Vec::new(),
            out_of_scope_action_attempted: false,
            out_of_scope_denied: false,
            mcp_effects: Vec::new(),
            cli_effects: Vec::new(),
            transport_parity_match: None,
        }
    }
}

fn transition_allowed(from: LifecycleState, to: LifecycleState) -> bool {
    matches!(
        (from, to),
        (LifecycleState::New, LifecycleState::Planned)
            | (LifecycleState::Planned, LifecycleState::Active)
            | (
                LifecycleState::Active,
                LifecycleState::Completed | LifecycleState::Cancelled
            )
    )
}

fn submit_lifecycle_transition(
    world: &mut Phase0World,
    next: LifecycleState,
) -> Result<(), MutationErrorCode> {
    if !transition_allowed(world.lifecycle_state, next) {
        world.last_mutation_error = Some(MutationErrorCode::InvalidTransition);
        return Err(MutationErrorCode::InvalidTransition);
    }

    world.lifecycle_state = next;
    world.lifecycle_trace.push(next);
    world.query_surface_status = next;
    world.persisted_revision += 1;
    world.last_mutation_error = None;
    Ok(())
}

fn parse_replay_event(event: &str) -> Option<String> {
    let status = event.strip_prefix("status:")?;
    if matches!(status, "planned" | "active" | "completed" | "cancelled") {
        return Some(event.to_string());
    }
    None
}

fn replay_history(world: &mut Phase0World) {
    let mut scratch = Vec::new();

    for (index, event) in world.replay_source_history.iter().enumerate() {
        let Some(parsed_event) = parse_replay_event(event) else {
            world.replay_error = Some(format!("invalid_event_at_index:{index}"));
            world.replayed_snapshot.clear();
            world.replay_applied_count = 0;
            return;
        };
        scratch.push(parsed_event);
    }

    world.replay_error = None;
    world.replayed_snapshot = scratch;
    world.replay_applied_count = world.replayed_snapshot.len();
}

fn auth_error_label(error: AuthErrorCode) -> &'static str {
    if error == AuthErrorCode::InvalidIdentity {
        "invalid_identity"
    } else {
        "replay_credential"
    }
}

fn authorize(world: &Phase0World, replay_credential_used: bool) -> Result<(), AuthErrorCode> {
    if !world.actor_identity_valid {
        return Err(AuthErrorCode::InvalidIdentity);
    }
    if replay_credential_used || !world.replay_credential_fresh {
        return Err(AuthErrorCode::ReplayCredential);
    }
    Ok(())
}

fn execute_protected_command(
    world: &mut Phase0World,
    command: &str,
    replay_credential_used: bool,
) -> Result<(), AuthErrorCode> {
    authorize(world, replay_credential_used)?;

    match command {
        "create" => {
            if world.lifecycle_state == LifecycleState::New {
                world.lifecycle_state = LifecycleState::Active;
                world.protected_mutation_count += 1;
                world.command_outcomes.push("create:accepted".to_string());
            } else {
                world
                    .command_outcomes
                    .push("create:rejected:invalid_transition".to_string());
            }
        }
        "inspect" => world.command_outcomes.push("inspect:accepted".to_string()),
        "list" => world.command_outcomes.push("list:accepted".to_string()),
        "cancel" => {
            if world.lifecycle_state == LifecycleState::Active {
                world.lifecycle_state = LifecycleState::Cancelled;
                world.protected_mutation_count += 1;
                world.command_outcomes.push("cancel:accepted".to_string());
            } else {
                world
                    .command_outcomes
                    .push("cancel:rejected:invalid_transition".to_string());
            }
        }
        _ => world
            .command_outcomes
            .push("unknown:rejected:unsupported_command".to_string()),
    }

    Ok(())
}

#[given("a new project state")]
fn given_new_project_state(world: &mut Phase0World) {
    world.lifecycle_state = LifecycleState::New;
    world.lifecycle_trace = vec![LifecycleState::New];
    world.query_surface_status = LifecycleState::New;
    world.last_mutation_error = None;
    world.persisted_revision = 0;
    world.persisted_revision_before_attempt = 0;
}

#[when("a valid sequence of lifecycle mutations is submitted")]
fn when_valid_lifecycle_sequence_submitted(world: &mut Phase0World) {
    for next in [
        LifecycleState::Planned,
        LifecycleState::Active,
        LifecycleState::Completed,
    ] {
        let result = submit_lifecycle_transition(world, next);
        assert!(result.is_ok());
    }
}

#[then("state advances predictably according to declared transition rules")]
fn then_state_advances_predictably(world: &mut Phase0World) {
    assert_eq!(
        world.lifecycle_trace,
        vec![
            LifecycleState::New,
            LifecycleState::Planned,
            LifecycleState::Active,
            LifecycleState::Completed,
        ]
    );
    assert_eq!(world.lifecycle_state, LifecycleState::Completed);
}

#[then("resulting status is consistent across query surfaces")]
fn then_status_consistent_across_query_surfaces(world: &mut Phase0World) {
    assert_eq!(world.lifecycle_state, world.query_surface_status);
}

#[given("an entity already in a terminal or incompatible state")]
fn given_terminal_or_incompatible_state(world: &mut Phase0World) {
    world.lifecycle_state = LifecycleState::Completed;
    world.lifecycle_trace = vec![
        LifecycleState::New,
        LifecycleState::Planned,
        LifecycleState::Active,
        LifecycleState::Completed,
    ];
    world.query_surface_status = LifecycleState::Completed;
    world.last_mutation_error = None;
    world.persisted_revision = 3;
    world.persisted_revision_before_attempt = world.persisted_revision;
}

#[when("an illegal transition is attempted")]
fn when_illegal_transition_attempted(world: &mut Phase0World) {
    let result = submit_lifecycle_transition(world, LifecycleState::Active);
    assert!(result.is_err());
}

#[then("the operation is rejected with a typed error")]
fn then_operation_rejected_with_typed_error(world: &mut Phase0World) {
    assert_eq!(
        world.last_mutation_error,
        Some(MutationErrorCode::InvalidTransition)
    );
}

#[then("no partial state mutation is persisted")]
fn then_no_partial_state_mutation_persisted(world: &mut Phase0World) {
    assert_eq!(world.lifecycle_state, LifecycleState::Completed);
    assert_eq!(world.query_surface_status, LifecycleState::Completed);
    assert_eq!(
        world.persisted_revision,
        world.persisted_revision_before_attempt
    );
}

#[given("a successful mutation")]
fn given_successful_mutation(world: &mut Phase0World) {
    world.successful_mutation_pending = true;
    world.event_history.clear();
    world.projection_snapshot.clear();
    world.operational_snapshot = vec!["status:active".to_string()];
}

#[when("the mutation commits")]
fn when_mutation_commits(world: &mut Phase0World) {
    if world.successful_mutation_pending {
        world.event_history.push("event:status:active".to_string());
        world.projection_snapshot = world.operational_snapshot.clone();
        world.successful_mutation_pending = false;
    }
}

#[then("its event(s) are appended durably")]
fn then_events_appended_durably(world: &mut Phase0World) {
    assert!(!world.event_history.is_empty());
}

#[then("projections/read-models reflect the same committed truth")]
fn then_projections_match_committed_truth(world: &mut Phase0World) {
    assert_eq!(world.projection_snapshot, world.operational_snapshot);
}

#[given("a committed event history")]
fn given_committed_event_history(world: &mut Phase0World) {
    world.replay_source_history = vec![
        "status:planned".to_string(),
        "status:active".to_string(),
        "status:completed".to_string(),
    ];
    world.operational_snapshot = world.replay_source_history.clone();
    world.replayed_snapshot.clear();
    world.replay_error = None;
    world.replay_applied_count = 0;
}

#[when("state is rebuilt from replay into a clean store")]
fn when_state_rebuilt_from_replay(world: &mut Phase0World) {
    replay_history(world);
}

#[then("reconstructed state matches the original operational state")]
fn then_reconstructed_state_matches(world: &mut Phase0World) {
    assert_eq!(world.replay_error, None);
    assert_eq!(world.replayed_snapshot, world.operational_snapshot);
}

#[given("malformed or semantically invalid event input")]
fn given_invalid_event_input(world: &mut Phase0World) {
    world.replay_source_history = vec![
        "status:planned".to_string(),
        "status:INVALID".to_string(),
        "status:completed".to_string(),
    ];
    world.replayed_snapshot.clear();
    world.replay_error = None;
    world.replay_applied_count = 0;
}

#[when("replay is attempted")]
fn when_replay_attempted(world: &mut Phase0World) {
    replay_history(world);
}

#[then("replay fails with explicit diagnostics")]
fn then_replay_fails_with_diagnostics(world: &mut Phase0World) {
    let error_prefix = world.replay_error.as_deref().unwrap_or_default();
    assert!(error_prefix.starts_with("invalid_event_at_index:"));
}

#[then("no partial replay is left behind")]
fn then_no_partial_replay(world: &mut Phase0World) {
    assert!(world.replayed_snapshot.is_empty());
    assert_eq!(world.replay_applied_count, 0);
}

#[given("a configured environment and valid actor identity")]
fn given_valid_actor_identity(world: &mut Phase0World) {
    world.actor_identity_valid = true;
    world.replay_credential_fresh = true;
    world.lifecycle_state = LifecycleState::New;
    world.command_outcomes.clear();
    world.auth_errors.clear();
    world.protected_mutation_count = 0;
}

#[when("an operator performs create, inspect, list, and cancel flows")]
fn when_operator_runs_dispatch_flows(world: &mut Phase0World) {
    let create_result = execute_protected_command(world, "create", false);
    assert!(create_result.is_ok());
    let inspect_result = execute_protected_command(world, "inspect", false);
    assert!(inspect_result.is_ok());
    let list_result = execute_protected_command(world, "list", false);
    assert!(list_result.is_ok());
    let cancel_result = execute_protected_command(world, "cancel", false);
    assert!(cancel_result.is_ok());
    let repeated_cancel_result = execute_protected_command(world, "cancel", false);
    assert!(repeated_cancel_result.is_ok());
}

#[then("each action is accepted/rejected according to policy and lifecycle rules")]
fn then_actions_follow_policy_and_lifecycle_rules(world: &mut Phase0World) {
    let expected = [
        "create:accepted",
        "inspect:accepted",
        "list:accepted",
        "cancel:accepted",
        "cancel:rejected:invalid_transition",
    ]
    .iter()
    .map(|entry| (*entry).to_string())
    .collect::<Vec<_>>();
    assert_eq!(world.command_outcomes, expected);
}

#[then("observed outcomes match the same domain contract semantics")]
fn then_outcomes_match_domain_contract(world: &mut Phase0World) {
    assert_eq!(world.lifecycle_state, LifecycleState::Cancelled);
    assert_eq!(world.protected_mutation_count, 2);
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let feature_path = env::var("TANREN_BDD_PHASE0_FEATURE_PATH")
        .unwrap_or_else(|_| "tests/bdd/phase0/smoke.feature".to_string());

    Phase0World::cucumber()
        .max_concurrent_scenarios(1)
        .fail_on_skipped()
        .run_and_exit(feature_path)
        .await;
}

#[cfg(test)]
mod tests;
