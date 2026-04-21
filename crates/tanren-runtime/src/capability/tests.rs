use super::*;

fn baseline() -> HarnessCapabilities {
    HarnessCapabilities {
        output_streaming: OutputStreaming::TextAndToolEvents,
        can_use_tools: true,
        patch_apply: PatchApplySupport::ApplyPatchAndUnifiedDiff,
        session_resume: SessionResumeSupport::CrossProcess,
        sandbox_mode: SandboxMode::WorkspaceWrite,
        approval_mode: ApprovalMode::OnDemand,
    }
}

#[test]
fn baseline_is_admissible_for_default_requirements() {
    let requirements = HarnessRequirements::default();
    assert_eq!(
        baseline().evaluate(&requirements),
        CapabilityAdmissibility::Admissible
    );
}

#[test]
fn denies_text_streaming_requirement_when_output_streaming_is_none() {
    let caps = HarnessCapabilities {
        output_streaming: OutputStreaming::None,
        ..baseline()
    };
    let requirements = HarnessRequirements {
        output_streaming: OutputStreamingRequirement::Text,
        ..HarnessRequirements::default()
    };
    let denial = caps
        .ensure_admissible(&requirements)
        .expect_err("must deny text requirement");
    assert_eq!(
        denial.kind,
        CompatibilityDenialKind::TextStreamingUnsupported
    );
}

#[test]
fn denies_tool_event_streaming_requirement_without_tool_events() {
    let caps = HarnessCapabilities {
        output_streaming: OutputStreaming::TextDeltas,
        ..baseline()
    };
    let requirements = HarnessRequirements {
        output_streaming: OutputStreamingRequirement::TextAndToolEvents,
        ..HarnessRequirements::default()
    };
    let denial = caps
        .ensure_admissible(&requirements)
        .expect_err("must deny tool-events requirement");
    assert_eq!(
        denial.kind,
        CompatibilityDenialKind::ToolEventStreamingUnsupported
    );
}

#[test]
fn allows_text_requirement_with_text_deltas() {
    let caps = HarnessCapabilities {
        output_streaming: OutputStreaming::TextDeltas,
        ..baseline()
    };
    let requirements = HarnessRequirements {
        output_streaming: OutputStreamingRequirement::Text,
        ..HarnessRequirements::default()
    };
    assert!(caps.ensure_admissible(&requirements).is_ok());
}

#[test]
fn denies_when_tool_use_is_required_but_missing() {
    let mut caps = baseline();
    caps.can_use_tools = false;
    let requirements = HarnessRequirements {
        tool_use: RequirementLevel::Required,
        ..HarnessRequirements::default()
    };
    let denial = caps
        .ensure_admissible(&requirements)
        .expect_err("must deny");
    assert_eq!(denial.kind, CompatibilityDenialKind::ToolUseUnsupported);
}

#[test]
fn denies_when_patch_apply_level_is_insufficient() {
    let caps = HarnessCapabilities {
        patch_apply: PatchApplySupport::ApplyPatchOnly,
        ..baseline()
    };
    let requirements = HarnessRequirements {
        patch_apply: PatchApplyRequirement::ApplyPatchAndUnifiedDiff,
        ..HarnessRequirements::default()
    };
    let denial = caps
        .ensure_admissible(&requirements)
        .expect_err("must deny");
    assert_eq!(
        denial.kind,
        CompatibilityDenialKind::PatchApplyLevelInsufficient
    );
}

#[test]
fn denies_when_session_resume_level_is_insufficient() {
    let caps = HarnessCapabilities {
        session_resume: SessionResumeSupport::SameProcessOnly,
        ..baseline()
    };
    let requirements = HarnessRequirements {
        session_resume: SessionResumeRequirement::CrossProcess,
        ..HarnessRequirements::default()
    };
    let denial = caps
        .ensure_admissible(&requirements)
        .expect_err("must deny");
    assert_eq!(
        denial.kind,
        CompatibilityDenialKind::SessionResumeLevelInsufficient
    );
}

#[test]
fn allows_cross_process_for_same_process_requirement() {
    let requirements = HarnessRequirements {
        session_resume: SessionResumeRequirement::SameProcessOnly,
        ..HarnessRequirements::default()
    };
    assert!(baseline().ensure_admissible(&requirements).is_ok());
}

#[test]
fn denies_when_sandbox_mode_is_below_minimum() {
    let caps = HarnessCapabilities {
        sandbox_mode: SandboxMode::ReadOnly,
        ..baseline()
    };
    let requirements = HarnessRequirements {
        minimum_sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        ..HarnessRequirements::default()
    };
    let denial = caps
        .ensure_admissible(&requirements)
        .expect_err("must deny");
    assert_eq!(
        denial.kind,
        CompatibilityDenialKind::SandboxModeBelowMinimum
    );
}

#[test]
fn denies_when_sandbox_mode_exceeds_maximum() {
    let caps = HarnessCapabilities {
        sandbox_mode: SandboxMode::Unrestricted,
        ..baseline()
    };
    let requirements = HarnessRequirements {
        maximum_sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        ..HarnessRequirements::default()
    };
    let denial = caps
        .ensure_admissible(&requirements)
        .expect_err("must deny");
    assert_eq!(
        denial.kind,
        CompatibilityDenialKind::SandboxModeExceedsMaximum
    );
}

#[test]
fn denies_when_sandbox_range_is_invalid() {
    let requirements = HarnessRequirements {
        minimum_sandbox_mode: Some(SandboxMode::Unrestricted),
        maximum_sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        ..HarnessRequirements::default()
    };
    let denial = baseline()
        .ensure_admissible(&requirements)
        .expect_err("must deny invalid range");
    assert_eq!(
        denial.kind,
        CompatibilityDenialKind::SandboxModeInvalidRange
    );
}

#[test]
fn denies_when_approval_mode_is_below_minimum() {
    let caps = HarnessCapabilities {
        approval_mode: ApprovalMode::OnEscalation,
        ..baseline()
    };
    let requirements = HarnessRequirements {
        minimum_approval_mode: Some(ApprovalMode::OnDemand),
        ..HarnessRequirements::default()
    };
    let denial = caps
        .ensure_admissible(&requirements)
        .expect_err("must deny");
    assert_eq!(
        denial.kind,
        CompatibilityDenialKind::ApprovalModeBelowMinimum
    );
}

#[test]
fn denies_when_approval_mode_exceeds_maximum() {
    let caps = HarnessCapabilities {
        approval_mode: ApprovalMode::Never,
        ..baseline()
    };
    let requirements = HarnessRequirements {
        maximum_approval_mode: Some(ApprovalMode::OnEscalation),
        ..HarnessRequirements::default()
    };
    let denial = caps
        .ensure_admissible(&requirements)
        .expect_err("must deny");
    assert_eq!(
        denial.kind,
        CompatibilityDenialKind::ApprovalModeExceedsMaximum
    );
}

#[test]
fn approval_minimum_and_maximum_can_coexist_with_dual_ordering() {
    let requirements = HarnessRequirements {
        minimum_approval_mode: Some(ApprovalMode::OnDemand),
        maximum_approval_mode: Some(ApprovalMode::OnEscalation),
        ..HarnessRequirements::default()
    };
    let caps = HarnessCapabilities {
        approval_mode: ApprovalMode::OnDemand,
        ..baseline()
    };
    assert!(caps.ensure_admissible(&requirements).is_ok());
}

#[test]
fn allows_when_modes_are_within_min_max_bounds() {
    let requirements = HarnessRequirements {
        minimum_sandbox_mode: Some(SandboxMode::ReadOnly),
        maximum_sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        minimum_approval_mode: Some(ApprovalMode::OnEscalation),
        maximum_approval_mode: Some(ApprovalMode::OnEscalation),
        ..HarnessRequirements::default()
    };
    assert!(baseline().ensure_admissible(&requirements).is_ok());
}

fn strictness_rank(mode: ApprovalMode) -> u8 {
    match mode {
        ApprovalMode::Never => 0,
        ApprovalMode::OnEscalation => 1,
        ApprovalMode::OnDemand => 2,
    }
}

fn privilege_rank(mode: ApprovalMode) -> u8 {
    match mode {
        ApprovalMode::OnDemand => 0,
        ApprovalMode::OnEscalation => 1,
        ApprovalMode::Never => 2,
    }
}

fn expected_approval_denial(
    actual: ApprovalMode,
    minimum: Option<ApprovalMode>,
    maximum: Option<ApprovalMode>,
) -> Option<CompatibilityDenialKind> {
    if let Some(minimum_mode) = minimum
        && strictness_rank(actual) < strictness_rank(minimum_mode)
    {
        return Some(CompatibilityDenialKind::ApprovalModeBelowMinimum);
    }
    if let Some(maximum_mode) = maximum
        && privilege_rank(actual) > privilege_rank(maximum_mode)
    {
        return Some(CompatibilityDenialKind::ApprovalModeExceedsMaximum);
    }
    None
}

#[test]
fn approval_mode_dual_ordering_matrix_is_exhaustive() {
    let modes = [
        ApprovalMode::Never,
        ApprovalMode::OnEscalation,
        ApprovalMode::OnDemand,
    ];
    let bounds = [
        None,
        Some(ApprovalMode::Never),
        Some(ApprovalMode::OnEscalation),
        Some(ApprovalMode::OnDemand),
    ];

    for actual in modes {
        for minimum in bounds {
            for maximum in bounds {
                let caps = HarnessCapabilities {
                    approval_mode: actual,
                    ..baseline()
                };
                let requirements = HarnessRequirements {
                    minimum_approval_mode: minimum,
                    maximum_approval_mode: maximum,
                    ..HarnessRequirements::default()
                };
                let expected = expected_approval_denial(actual, minimum, maximum);
                let result = caps.ensure_admissible(&requirements);
                match expected {
                    Some(expected_kind) => {
                        let denial = result.expect_err("expected denial");
                        assert_eq!(
                            denial.kind, expected_kind,
                            "actual={actual:?} minimum={minimum:?} maximum={maximum:?}"
                        );
                    }
                    None => {
                        assert!(
                            result.is_ok(),
                            "actual={actual:?} minimum={minimum:?} maximum={maximum:?}"
                        );
                    }
                }
            }
        }
    }
}
