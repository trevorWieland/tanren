use super::*;

#[test]
fn submit_lifecycle_transition_accepts_valid_transition_and_updates_revision() {
    let mut world = Phase0World {
        lifecycle_state: LifecycleState::New,
        lifecycle_trace: vec![LifecycleState::New],
        query_surface_status: LifecycleState::New,
        persisted_revision: 2,
        ..Default::default()
    };

    let result = submit_lifecycle_transition(&mut world, LifecycleState::Planned);
    assert!(result.is_ok());
    assert_eq!(world.lifecycle_state, LifecycleState::Planned);
    assert_eq!(world.query_surface_status, LifecycleState::Planned);
    assert_eq!(world.persisted_revision, 3);
}

#[test]
fn submit_lifecycle_transition_rejects_invalid_transition_without_mutating_revision() {
    let mut world = Phase0World {
        lifecycle_state: LifecycleState::Completed,
        lifecycle_trace: vec![
            LifecycleState::New,
            LifecycleState::Planned,
            LifecycleState::Active,
            LifecycleState::Completed,
        ],
        query_surface_status: LifecycleState::Completed,
        persisted_revision: 7,
        ..Default::default()
    };

    let result = submit_lifecycle_transition(&mut world, LifecycleState::Active);
    assert_eq!(result, Err(MutationErrorCode::InvalidTransition));
    assert_eq!(world.lifecycle_state, LifecycleState::Completed);
    assert_eq!(world.query_surface_status, LifecycleState::Completed);
    assert_eq!(world.persisted_revision, 7);
}
