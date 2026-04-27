use serde::{Deserialize, Serialize};

use super::{ApprovalMode, SandboxMode};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum RequirementBoundsError {
    #[error("sandbox minimum exceeds maximum")]
    SandboxModeInvalidRange,
}

/// Validated sandbox bound requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
pub struct SandboxModeBounds {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "minimum_sandbox_mode",
        alias = "required_sandbox_mode"
    )]
    minimum: Option<SandboxMode>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "maximum_sandbox_mode"
    )]
    maximum: Option<SandboxMode>,
}

impl SandboxModeBounds {
    /// # Errors
    /// Returns [`RequirementBoundsError::SandboxModeInvalidRange`] when invalid.
    pub fn try_new(
        minimum: Option<SandboxMode>,
        maximum: Option<SandboxMode>,
    ) -> Result<Self, RequirementBoundsError> {
        if let (Some(minimum), Some(maximum)) = (minimum, maximum)
            && sandbox_mode_rank(minimum) > sandbox_mode_rank(maximum)
        {
            return Err(RequirementBoundsError::SandboxModeInvalidRange);
        }
        Ok(Self { minimum, maximum })
    }

    #[must_use]
    pub const fn minimum(self) -> Option<SandboxMode> {
        self.minimum
    }

    #[must_use]
    pub const fn maximum(self) -> Option<SandboxMode> {
        self.maximum
    }
}

#[derive(Deserialize)]
struct SandboxModeBoundsWire {
    #[serde(
        default,
        alias = "required_sandbox_mode",
        rename = "minimum_sandbox_mode"
    )]
    minimum: Option<SandboxMode>,
    #[serde(default, rename = "maximum_sandbox_mode")]
    maximum: Option<SandboxMode>,
}

impl<'de> Deserialize<'de> for SandboxModeBounds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = SandboxModeBoundsWire::deserialize(deserializer)?;
        Self::try_new(wire.minimum, wire.maximum).map_err(serde::de::Error::custom)
    }
}

/// Validated approval bound requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
pub struct ApprovalModeBounds {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "minimum_approval_mode",
        alias = "required_approval_mode"
    )]
    minimum: Option<ApprovalMode>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "maximum_approval_mode"
    )]
    maximum: Option<ApprovalMode>,
}

impl ApprovalModeBounds {
    #[must_use]
    pub const fn new(minimum: Option<ApprovalMode>, maximum: Option<ApprovalMode>) -> Self {
        Self { minimum, maximum }
    }

    #[must_use]
    pub const fn minimum(self) -> Option<ApprovalMode> {
        self.minimum
    }

    #[must_use]
    pub const fn maximum(self) -> Option<ApprovalMode> {
        self.maximum
    }
}

#[derive(Deserialize)]
struct ApprovalModeBoundsWire {
    #[serde(
        default,
        alias = "required_approval_mode",
        rename = "minimum_approval_mode"
    )]
    minimum: Option<ApprovalMode>,
    #[serde(default, rename = "maximum_approval_mode")]
    maximum: Option<ApprovalMode>,
}

impl<'de> Deserialize<'de> for ApprovalModeBounds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ApprovalModeBoundsWire::deserialize(deserializer)?;
        Ok(Self::new(wire.minimum, wire.maximum))
    }
}

/// Pre-execution requirements a dispatch needs from the selected harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessRequirements {
    #[serde(default)]
    output_streaming: OutputStreamingRequirement,
    #[serde(default)]
    tool_use: RequirementLevel,
    #[serde(default)]
    patch_apply: PatchApplyRequirement,
    #[serde(default)]
    session_resume: SessionResumeRequirement,
    #[serde(default, flatten)]
    sandbox_mode_bounds: SandboxModeBounds,
    #[serde(default, flatten)]
    approval_mode_bounds: ApprovalModeBounds,
}

impl Default for HarnessRequirements {
    fn default() -> Self {
        Self {
            output_streaming: OutputStreamingRequirement::None,
            tool_use: RequirementLevel::Optional,
            patch_apply: PatchApplyRequirement::None,
            session_resume: SessionResumeRequirement::None,
            sandbox_mode_bounds: SandboxModeBounds::default(),
            approval_mode_bounds: ApprovalModeBounds::default(),
        }
    }
}

impl HarnessRequirements {
    #[must_use]
    pub fn builder() -> HarnessRequirementsBuilder {
        HarnessRequirementsBuilder::default()
    }

    #[must_use]
    pub const fn output_streaming(&self) -> OutputStreamingRequirement {
        self.output_streaming
    }

    #[must_use]
    pub const fn tool_use(&self) -> RequirementLevel {
        self.tool_use
    }

    #[must_use]
    pub const fn patch_apply(&self) -> PatchApplyRequirement {
        self.patch_apply
    }

    #[must_use]
    pub const fn session_resume(&self) -> SessionResumeRequirement {
        self.session_resume
    }

    #[must_use]
    pub const fn sandbox_mode_bounds(&self) -> SandboxModeBounds {
        self.sandbox_mode_bounds
    }

    #[must_use]
    pub const fn approval_mode_bounds(&self) -> ApprovalModeBounds {
        self.approval_mode_bounds
    }
}

#[derive(Debug, Clone, Default)]
pub struct HarnessRequirementsBuilder {
    requirements: HarnessRequirements,
}

impl HarnessRequirementsBuilder {
    #[must_use]
    pub fn output_streaming(mut self, requirement: OutputStreamingRequirement) -> Self {
        self.requirements.output_streaming = requirement;
        self
    }

    #[must_use]
    pub fn tool_use(mut self, requirement: RequirementLevel) -> Self {
        self.requirements.tool_use = requirement;
        self
    }

    #[must_use]
    pub fn patch_apply(mut self, requirement: PatchApplyRequirement) -> Self {
        self.requirements.patch_apply = requirement;
        self
    }

    #[must_use]
    pub fn session_resume(mut self, requirement: SessionResumeRequirement) -> Self {
        self.requirements.session_resume = requirement;
        self
    }

    /// # Errors
    /// Returns [`RequirementBoundsError`] when bounds are malformed.
    pub fn sandbox_mode_bounds(
        mut self,
        minimum: Option<SandboxMode>,
        maximum: Option<SandboxMode>,
    ) -> Result<Self, RequirementBoundsError> {
        self.requirements.sandbox_mode_bounds = SandboxModeBounds::try_new(minimum, maximum)?;
        Ok(self)
    }

    #[must_use]
    pub fn approval_mode_bounds(
        mut self,
        minimum: Option<ApprovalMode>,
        maximum: Option<ApprovalMode>,
    ) -> Self {
        self.requirements.approval_mode_bounds = ApprovalModeBounds::new(minimum, maximum);
        self
    }

    #[must_use]
    pub fn build(self) -> HarnessRequirements {
        self.requirements
    }
}

const fn sandbox_mode_rank(mode: SandboxMode) -> u8 {
    match mode {
        SandboxMode::ReadOnly => 0,
        SandboxMode::WorkspaceWrite => 1,
        SandboxMode::Unrestricted => 2,
    }
}
