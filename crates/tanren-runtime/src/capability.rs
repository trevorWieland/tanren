use serde::{Deserialize, Serialize};

/// Output streaming support advertised by a harness adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStreaming {
    None,
    TextDeltas,
    TextAndToolEvents,
}

impl OutputStreaming {
    #[must_use]
    pub const fn supports_text(self) -> bool {
        !matches!(self, Self::None)
    }

    #[must_use]
    pub const fn supports_tool_events(self) -> bool {
        matches!(self, Self::TextAndToolEvents)
    }
}

/// Whether the harness supports applying patches natively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplySupport {
    Unsupported,
    ApplyPatchOnly,
    ApplyPatchAndUnifiedDiff,
}

/// Session behavior supported by the harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionResumeSupport {
    Never,
    SameProcessOnly,
    CrossProcess,
}

/// Sandbox behavior normalized across providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    Unrestricted,
}

/// Human approval behavior for restricted actions.
///
/// Ordering semantics are explicit and dual-axis:
/// - minimum bounds use strictness (`never` < `on_escalation` < `on_demand`)
/// - maximum bounds use privilege risk (`on_demand` < `on_escalation` < `never`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    Never,
    OnEscalation,
    OnDemand,
}

/// Capabilities a harness adapter advertises.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessCapabilities {
    pub output_streaming: OutputStreaming,
    pub can_use_tools: bool,
    pub patch_apply: PatchApplySupport,
    pub session_resume: SessionResumeSupport,
    pub sandbox_mode: SandboxMode,
    pub approval_mode: ApprovalMode,
}

/// Requirement strictness for one capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RequirementLevel {
    #[default]
    Optional,
    Required,
}

impl RequirementLevel {
    #[must_use]
    pub const fn is_required(self) -> bool {
        matches!(self, Self::Required)
    }
}

/// Required output-streaming class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputStreamingRequirement {
    #[default]
    None,
    Text,
    TextAndToolEvents,
}

/// Minimum patch-apply support required for admissibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PatchApplyRequirement {
    #[default]
    None,
    ApplyPatchOnly,
    ApplyPatchAndUnifiedDiff,
}

/// Minimum session-resume support required for admissibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionResumeRequirement {
    #[default]
    None,
    SameProcessOnly,
    CrossProcess,
}

/// Pre-execution requirements a dispatch needs from the selected harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HarnessRequirements {
    #[serde(default)]
    pub output_streaming: OutputStreamingRequirement,
    #[serde(default)]
    pub tool_use: RequirementLevel,
    #[serde(default)]
    pub patch_apply: PatchApplyRequirement,
    #[serde(default)]
    pub session_resume: SessionResumeRequirement,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "required_sandbox_mode"
    )]
    pub minimum_sandbox_mode: Option<SandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_sandbox_mode: Option<SandboxMode>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        alias = "required_approval_mode"
    )]
    pub minimum_approval_mode: Option<ApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_approval_mode: Option<ApprovalMode>,
}

/// Typed mismatch classes used for deterministic denial handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityDenialKind {
    TextStreamingUnsupported,
    ToolEventStreamingUnsupported,
    ToolUseUnsupported,
    PatchApplyLevelInsufficient,
    SessionResumeLevelInsufficient,
    SandboxModeBelowMinimum,
    SandboxModeExceedsMaximum,
    SandboxModeInvalidRange,
    ApprovalModeBelowMinimum,
    ApprovalModeExceedsMaximum,
    ApprovalModeInvalidRange,
}

/// Typed denial returned when requirements do not match adapter capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("capability mismatch: {kind:?}")]
pub struct CompatibilityDenial {
    pub kind: CompatibilityDenialKind,
}

/// Compatibility verdict returned by preflight checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityAdmissibility {
    Admissible,
    Denied(CompatibilityDenialKind),
}

impl HarnessCapabilities {
    /// Evaluate whether this adapter can satisfy the provided requirements.
    #[must_use]
    pub fn evaluate(&self, requirements: &HarnessRequirements) -> CapabilityAdmissibility {
        match requirements.output_streaming {
            OutputStreamingRequirement::None => {}
            OutputStreamingRequirement::Text => {
                if !self.output_streaming.supports_text() {
                    return CapabilityAdmissibility::Denied(
                        CompatibilityDenialKind::TextStreamingUnsupported,
                    );
                }
            }
            OutputStreamingRequirement::TextAndToolEvents => {
                if !self.output_streaming.supports_tool_events() {
                    return CapabilityAdmissibility::Denied(
                        CompatibilityDenialKind::ToolEventStreamingUnsupported,
                    );
                }
            }
        }

        if requirements.tool_use.is_required() && !self.can_use_tools {
            return CapabilityAdmissibility::Denied(CompatibilityDenialKind::ToolUseUnsupported);
        }

        if !patch_apply_satisfies(self.patch_apply, requirements.patch_apply) {
            return CapabilityAdmissibility::Denied(
                CompatibilityDenialKind::PatchApplyLevelInsufficient,
            );
        }

        if !session_resume_satisfies(self.session_resume, requirements.session_resume) {
            return CapabilityAdmissibility::Denied(
                CompatibilityDenialKind::SessionResumeLevelInsufficient,
            );
        }

        if let (Some(minimum), Some(maximum)) = (
            requirements.minimum_sandbox_mode,
            requirements.maximum_sandbox_mode,
        ) && sandbox_mode_rank(minimum) > sandbox_mode_rank(maximum)
        {
            return CapabilityAdmissibility::Denied(
                CompatibilityDenialKind::SandboxModeInvalidRange,
            );
        }
        if let Some(minimum) = requirements.minimum_sandbox_mode
            && !sandbox_mode_satisfies(self.sandbox_mode, minimum)
        {
            return CapabilityAdmissibility::Denied(
                CompatibilityDenialKind::SandboxModeBelowMinimum,
            );
        }
        if let Some(maximum) = requirements.maximum_sandbox_mode
            && sandbox_mode_rank(self.sandbox_mode) > sandbox_mode_rank(maximum)
        {
            return CapabilityAdmissibility::Denied(
                CompatibilityDenialKind::SandboxModeExceedsMaximum,
            );
        }

        if let (Some(minimum), Some(maximum)) = (
            requirements.minimum_approval_mode,
            requirements.maximum_approval_mode,
        ) && !approval_mode_range_has_solution(minimum, maximum)
        {
            return CapabilityAdmissibility::Denied(
                CompatibilityDenialKind::ApprovalModeInvalidRange,
            );
        }
        if let Some(minimum) = requirements.minimum_approval_mode
            && !approval_mode_satisfies_minimum(self.approval_mode, minimum)
        {
            return CapabilityAdmissibility::Denied(
                CompatibilityDenialKind::ApprovalModeBelowMinimum,
            );
        }
        if let Some(maximum) = requirements.maximum_approval_mode
            && !approval_mode_within_maximum(self.approval_mode, maximum)
        {
            return CapabilityAdmissibility::Denied(
                CompatibilityDenialKind::ApprovalModeExceedsMaximum,
            );
        }

        CapabilityAdmissibility::Admissible
    }

    /// Return `Ok(())` when admissible, otherwise a typed denial.
    ///
    /// # Errors
    /// Returns [`CompatibilityDenial`] if this adapter cannot satisfy the
    /// given requirements.
    pub fn ensure_admissible(
        &self,
        requirements: &HarnessRequirements,
    ) -> Result<(), CompatibilityDenial> {
        match self.evaluate(requirements) {
            CapabilityAdmissibility::Admissible => Ok(()),
            CapabilityAdmissibility::Denied(kind) => Err(CompatibilityDenial { kind }),
        }
    }
}

const fn patch_apply_support_rank(support: PatchApplySupport) -> u8 {
    match support {
        PatchApplySupport::Unsupported => 0,
        PatchApplySupport::ApplyPatchOnly => 1,
        PatchApplySupport::ApplyPatchAndUnifiedDiff => 2,
    }
}

const fn patch_apply_requirement_rank(requirement: PatchApplyRequirement) -> u8 {
    match requirement {
        PatchApplyRequirement::None => 0,
        PatchApplyRequirement::ApplyPatchOnly => 1,
        PatchApplyRequirement::ApplyPatchAndUnifiedDiff => 2,
    }
}

const fn patch_apply_satisfies(actual: PatchApplySupport, required: PatchApplyRequirement) -> bool {
    patch_apply_support_rank(actual) >= patch_apply_requirement_rank(required)
}

const fn session_resume_support_rank(support: SessionResumeSupport) -> u8 {
    match support {
        SessionResumeSupport::Never => 0,
        SessionResumeSupport::SameProcessOnly => 1,
        SessionResumeSupport::CrossProcess => 2,
    }
}

const fn session_resume_requirement_rank(requirement: SessionResumeRequirement) -> u8 {
    match requirement {
        SessionResumeRequirement::None => 0,
        SessionResumeRequirement::SameProcessOnly => 1,
        SessionResumeRequirement::CrossProcess => 2,
    }
}

const fn session_resume_satisfies(
    actual: SessionResumeSupport,
    required: SessionResumeRequirement,
) -> bool {
    session_resume_support_rank(actual) >= session_resume_requirement_rank(required)
}

const fn sandbox_mode_rank(mode: SandboxMode) -> u8 {
    match mode {
        SandboxMode::ReadOnly => 0,
        SandboxMode::WorkspaceWrite => 1,
        SandboxMode::Unrestricted => 2,
    }
}

const fn sandbox_mode_satisfies(actual: SandboxMode, required: SandboxMode) -> bool {
    sandbox_mode_rank(actual) >= sandbox_mode_rank(required)
}

const fn approval_strictness_rank(mode: ApprovalMode) -> u8 {
    match mode {
        ApprovalMode::Never => 0,
        ApprovalMode::OnEscalation => 1,
        ApprovalMode::OnDemand => 2,
    }
}

const fn approval_privilege_rank(mode: ApprovalMode) -> u8 {
    match mode {
        ApprovalMode::OnDemand => 0,
        ApprovalMode::OnEscalation => 1,
        ApprovalMode::Never => 2,
    }
}

const fn approval_mode_satisfies_minimum(actual: ApprovalMode, minimum: ApprovalMode) -> bool {
    approval_strictness_rank(actual) >= approval_strictness_rank(minimum)
}

const fn approval_mode_within_maximum(actual: ApprovalMode, maximum: ApprovalMode) -> bool {
    approval_privilege_rank(actual) <= approval_privilege_rank(maximum)
}

const fn approval_mode_range_has_solution(minimum: ApprovalMode, maximum: ApprovalMode) -> bool {
    approval_mode_satisfies_minimum(ApprovalMode::Never, minimum)
        && approval_mode_within_maximum(ApprovalMode::Never, maximum)
        || approval_mode_satisfies_minimum(ApprovalMode::OnEscalation, minimum)
            && approval_mode_within_maximum(ApprovalMode::OnEscalation, maximum)
        || approval_mode_satisfies_minimum(ApprovalMode::OnDemand, minimum)
            && approval_mode_within_maximum(ApprovalMode::OnDemand, maximum)
}

#[cfg(test)]
mod tests;
