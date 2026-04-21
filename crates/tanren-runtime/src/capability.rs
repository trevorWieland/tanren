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

impl PatchApplySupport {
    #[must_use]
    pub const fn supports_apply_patch(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// Session behavior supported by the harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionResumeSupport {
    Never,
    SameProcessOnly,
    CrossProcess,
}

impl SessionResumeSupport {
    #[must_use]
    pub const fn allows_resume(self) -> bool {
        !matches!(self, Self::Never)
    }
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

/// Pre-execution requirements a dispatch needs from the selected harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HarnessRequirements {
    #[serde(default)]
    pub output_streaming: OutputStreamingRequirement,
    #[serde(default)]
    pub tool_use: RequirementLevel,
    #[serde(default)]
    pub patch_apply: RequirementLevel,
    #[serde(default)]
    pub session_resume: RequirementLevel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_sandbox_mode: Option<SandboxMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_approval_mode: Option<ApprovalMode>,
}

/// Typed mismatch classes used for deterministic denial handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityDenialKind {
    TextStreamingUnsupported,
    ToolEventStreamingUnsupported,
    ToolUseUnsupported,
    PatchApplyUnsupported,
    SessionResumeUnsupported,
    SandboxModeInsufficient,
    ApprovalModeInsufficient,
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
        if requirements.patch_apply.is_required() && !self.patch_apply.supports_apply_patch() {
            return CapabilityAdmissibility::Denied(CompatibilityDenialKind::PatchApplyUnsupported);
        }
        if requirements.session_resume.is_required() && !self.session_resume.allows_resume() {
            return CapabilityAdmissibility::Denied(
                CompatibilityDenialKind::SessionResumeUnsupported,
            );
        }
        if let Some(required) = requirements.required_sandbox_mode {
            if !sandbox_mode_satisfies(self.sandbox_mode, required) {
                return CapabilityAdmissibility::Denied(
                    CompatibilityDenialKind::SandboxModeInsufficient,
                );
            }
        }
        if let Some(required) = requirements.required_approval_mode {
            if !approval_mode_satisfies(self.approval_mode, required) {
                return CapabilityAdmissibility::Denied(
                    CompatibilityDenialKind::ApprovalModeInsufficient,
                );
            }
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

const fn approval_mode_rank(mode: ApprovalMode) -> u8 {
    match mode {
        ApprovalMode::Never => 0,
        ApprovalMode::OnEscalation => 1,
        ApprovalMode::OnDemand => 2,
    }
}

const fn approval_mode_satisfies(actual: ApprovalMode, required: ApprovalMode) -> bool {
    approval_mode_rank(actual) >= approval_mode_rank(required)
}

#[cfg(test)]
mod tests {
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
    fn denies_when_patch_apply_is_required_but_missing() {
        let mut caps = baseline();
        caps.patch_apply = PatchApplySupport::Unsupported;
        let requirements = HarnessRequirements {
            patch_apply: RequirementLevel::Required,
            ..HarnessRequirements::default()
        };
        let denial = caps
            .ensure_admissible(&requirements)
            .expect_err("must deny");
        assert_eq!(denial.kind, CompatibilityDenialKind::PatchApplyUnsupported);
    }

    #[test]
    fn denies_when_sandbox_rank_is_insufficient() {
        let caps = HarnessCapabilities {
            sandbox_mode: SandboxMode::ReadOnly,
            ..baseline()
        };
        let requirements = HarnessRequirements {
            required_sandbox_mode: Some(SandboxMode::WorkspaceWrite),
            ..HarnessRequirements::default()
        };
        let denial = caps
            .ensure_admissible(&requirements)
            .expect_err("must deny");
        assert_eq!(
            denial.kind,
            CompatibilityDenialKind::SandboxModeInsufficient
        );
    }

    #[test]
    fn allows_stronger_sandbox_mode() {
        let caps = HarnessCapabilities {
            sandbox_mode: SandboxMode::Unrestricted,
            ..baseline()
        };
        let requirements = HarnessRequirements {
            required_sandbox_mode: Some(SandboxMode::WorkspaceWrite),
            ..HarnessRequirements::default()
        };
        assert!(caps.ensure_admissible(&requirements).is_ok());
    }
}
