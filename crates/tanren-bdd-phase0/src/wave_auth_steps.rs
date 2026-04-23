use cucumber::{given, then, when};

use crate::{
    AuthErrorCode, LifecycleState, Phase0World, auth_error_label, execute_protected_command,
};

#[given("invalid identity material or replayed mutation credentials")]
fn given_invalid_identity_or_replayed_credentials(world: &mut Phase0World) {
    world.actor_identity_valid = false;
    world.replay_credential_fresh = false;
    world.lifecycle_state = LifecycleState::New;
    world.command_outcomes.clear();
    world.auth_errors.clear();
    world.protected_mutation_count = 0;
}

#[when("a protected command is attempted")]
fn when_protected_command_attempted(world: &mut Phase0World) {
    let invalid_identity = execute_protected_command(world, "create", false);
    if let Err(error) = invalid_identity {
        world.auth_errors.push(error);
        world
            .command_outcomes
            .push(format!("create:auth_error:{}", auth_error_label(error)));
    }

    world.actor_identity_valid = true;
    let replay_attempt = execute_protected_command(world, "cancel", true);
    if let Err(error) = replay_attempt {
        world.auth_errors.push(error);
        world
            .command_outcomes
            .push(format!("cancel:auth_error:{}", auth_error_label(error)));
    }
}

#[then("the command is rejected with typed auth/replay errors")]
fn then_command_rejected_with_typed_auth_replay_errors(world: &mut Phase0World) {
    assert_eq!(
        world.auth_errors.as_slice(),
        &[
            AuthErrorCode::InvalidIdentity,
            AuthErrorCode::ReplayCredential
        ]
    );
}

#[then("no unintended mutation occurs")]
fn then_no_unintended_mutation_occurs(world: &mut Phase0World) {
    assert_eq!(world.protected_mutation_count, 0);
    assert_eq!(world.lifecycle_state, LifecycleState::New);
}
