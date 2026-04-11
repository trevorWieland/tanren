//! Property-based tests for lifecycle state machine invariants.

use proptest::prelude::*;
use tanren_domain::{DispatchStatus, LeaseStatus, StepStatus};

// ---------------------------------------------------------------------------
// Strategies
// ---------------------------------------------------------------------------

fn arb_dispatch_status() -> impl Strategy<Value = DispatchStatus> {
    prop_oneof![
        Just(DispatchStatus::Pending),
        Just(DispatchStatus::Running),
        Just(DispatchStatus::Completed),
        Just(DispatchStatus::Failed),
        Just(DispatchStatus::Cancelled),
    ]
}

fn arb_step_status() -> impl Strategy<Value = StepStatus> {
    prop_oneof![
        Just(StepStatus::Pending),
        Just(StepStatus::Running),
        Just(StepStatus::Completed),
        Just(StepStatus::Failed),
        Just(StepStatus::Cancelled),
    ]
}

fn arb_lease_status() -> impl Strategy<Value = LeaseStatus> {
    prop_oneof![
        Just(LeaseStatus::Requested),
        Just(LeaseStatus::Provisioning),
        Just(LeaseStatus::Ready),
        Just(LeaseStatus::Running),
        Just(LeaseStatus::Idle),
        Just(LeaseStatus::Draining),
        Just(LeaseStatus::Released),
        Just(LeaseStatus::Failed),
    ]
}

// ---------------------------------------------------------------------------
// Dispatch status properties
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn dispatch_terminal_has_no_outgoing(
        from in arb_dispatch_status().prop_filter(
            "terminal",
            |s| s.is_terminal()
        ),
        to in arb_dispatch_status(),
    ) {
        prop_assert!(
            !from.can_transition_to(to),
            "terminal {from} should not transition to {to}"
        );
    }

    #[test]
    fn dispatch_no_self_transitions(s in arb_dispatch_status()) {
        prop_assert!(
            !s.can_transition_to(s),
            "{s} should not self-transition"
        );
    }

    #[test]
    fn dispatch_is_terminal_consistent(
        from in arb_dispatch_status(),
        to in arb_dispatch_status(),
    ) {
        // If `from` is terminal, it cannot transition anywhere.
        if from.is_terminal() {
            prop_assert!(!from.can_transition_to(to));
        }
    }
}

// ---------------------------------------------------------------------------
// Step status properties
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn step_terminal_has_no_outgoing(
        from in arb_step_status().prop_filter(
            "terminal",
            |s| s.is_terminal()
        ),
        to in arb_step_status(),
    ) {
        prop_assert!(
            !from.can_transition_to(to),
            "terminal {from} should not transition to {to}"
        );
    }

    #[test]
    fn step_no_self_transitions(s in arb_step_status()) {
        prop_assert!(
            !s.can_transition_to(s),
            "{s} should not self-transition"
        );
    }

    #[test]
    fn step_is_terminal_consistent(
        from in arb_step_status(),
        to in arb_step_status(),
    ) {
        if from.is_terminal() {
            prop_assert!(!from.can_transition_to(to));
        }
    }
}

// ---------------------------------------------------------------------------
// Lease status properties
// ---------------------------------------------------------------------------

proptest! {
    #[test]
    fn lease_terminal_has_no_outgoing(
        from in arb_lease_status().prop_filter(
            "terminal",
            |s| s.is_terminal()
        ),
        to in arb_lease_status(),
    ) {
        prop_assert!(
            !from.can_transition_to(to),
            "terminal {from} should not transition to {to}"
        );
    }

    #[test]
    fn lease_is_terminal_consistent(
        from in arb_lease_status(),
        to in arb_lease_status(),
    ) {
        if from.is_terminal() {
            prop_assert!(!from.can_transition_to(to));
        }
    }

    #[test]
    fn lease_any_non_terminal_except_failed_can_fail(s in arb_lease_status()) {
        // Failed itself and Released cannot transition to Failed.
        // Every other state can enter Failed (post-failure cleanup
        // re-enters the drain path from there).
        if !s.is_terminal() && s != LeaseStatus::Failed {
            prop_assert!(
                s.can_transition_to(LeaseStatus::Failed),
                "non-terminal {s} should be able to fail"
            );
        }
    }

    #[test]
    fn lease_failed_is_not_terminal(
        to in arb_lease_status(),
    ) {
        // Regression: Failed must be able to reach Draining so
        // post-failure cleanup is explicit in the event history.
        prop_assert!(!LeaseStatus::Failed.is_terminal());
        if to == LeaseStatus::Draining {
            prop_assert!(LeaseStatus::Failed.can_transition_to(to));
        }
    }
}
