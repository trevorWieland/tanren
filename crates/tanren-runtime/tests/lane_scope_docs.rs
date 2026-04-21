#[test]
fn lane_1_1_scope_boundary_is_explicitly_documented() {
    let lane_1_1 = include_str!("../../../docs/rewrite/tasks/LANE-1.1-HARNESS.md");
    assert!(
        lane_1_1.contains("This lane does **not** ship concrete provider adapters."),
        "Lane 1.1 must explicitly state concrete adapter implementation is out of scope"
    );
    assert!(
        lane_1_1.contains(
            "Lane 1.1 audits must score adapter implementation completeness as out-of-scope"
        ),
        "Lane 1.1 must include explicit audit-scope guardrail"
    );
}

#[test]
fn lane_1_2_scope_requires_concrete_adapter_parity() {
    let lane_1_2 = include_str!("../../../docs/rewrite/tasks/LANE-1.2-BRIEF.md");
    assert!(
        lane_1_2.contains("All three Phase 1 harnesses are mandatory scope for this lane."),
        "Lane 1.2 must continue to require concrete adapter completeness"
    );
}
