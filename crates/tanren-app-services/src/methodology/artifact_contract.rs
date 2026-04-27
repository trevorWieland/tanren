//! Canonical spec-artifact ownership and protection contract.

/// `.tanren-generated-artifacts.json` manifest filename.
pub const GENERATED_ARTIFACT_MANIFEST_FILE: &str = ".tanren-generated-artifacts.json";
/// `.tanren-projection-checkpoint.json` checkpoint filename.
pub const PROJECTION_CHECKPOINT_FILE: &str = ".tanren-projection-checkpoint.json";

/// Protection mode applied during mutation sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactProtection {
    ReadOnly,
    AppendOnly,
    SessionWritable,
}

/// Canonical ownership row for one artifact file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactContractEntry {
    pub file: &'static str,
    pub protection: ArtifactProtection,
    pub included_in_generated_manifest: bool,
}

/// Single source of truth for methodology artifact ownership.
pub const ARTIFACT_CONTRACT: &[ArtifactContractEntry] = &[
    ArtifactContractEntry {
        file: "spec.md",
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: true,
    },
    ArtifactContractEntry {
        file: "plan.md",
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: true,
    },
    ArtifactContractEntry {
        file: "tasks.md",
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: true,
    },
    ArtifactContractEntry {
        file: "tasks.json",
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: true,
    },
    ArtifactContractEntry {
        file: "demo.md",
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: true,
    },
    ArtifactContractEntry {
        file: "audit.md",
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: true,
    },
    ArtifactContractEntry {
        file: "signposts.md",
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: true,
    },
    ArtifactContractEntry {
        file: "progress.json",
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: true,
    },
    ArtifactContractEntry {
        file: "phase-events.jsonl",
        protection: ArtifactProtection::AppendOnly,
        included_in_generated_manifest: true,
    },
    ArtifactContractEntry {
        file: GENERATED_ARTIFACT_MANIFEST_FILE,
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: false,
    },
    ArtifactContractEntry {
        file: PROJECTION_CHECKPOINT_FILE,
        protection: ArtifactProtection::ReadOnly,
        included_in_generated_manifest: false,
    },
    ArtifactContractEntry {
        file: "investigation-report.json",
        protection: ArtifactProtection::SessionWritable,
        included_in_generated_manifest: false,
    },
];

/// Ordered generated artifact set mirrored in `.tanren-generated-artifacts.json`.
#[must_use]
pub fn generated_manifest_artifacts() -> Vec<&'static str> {
    ARTIFACT_CONTRACT
        .iter()
        .filter(|entry| entry.included_in_generated_manifest)
        .map(|entry| entry.file)
        .collect()
}

/// Ordered readonly protected files for mutation-session enforcement.
#[must_use]
pub fn readonly_protected_artifacts() -> Vec<&'static str> {
    ARTIFACT_CONTRACT
        .iter()
        .filter(|entry| entry.protection == ArtifactProtection::ReadOnly)
        .map(|entry| entry.file)
        .collect()
}

/// Ordered append-only protected files for mutation-session enforcement.
#[must_use]
pub fn append_only_protected_artifacts() -> Vec<&'static str> {
    ARTIFACT_CONTRACT
        .iter()
        .filter(|entry| entry.protection == ArtifactProtection::AppendOnly)
        .map(|entry| entry.file)
        .collect()
}

/// Canonical readonly-banner text rendered into installed command assets.
#[must_use]
pub fn readonly_artifact_banner() -> String {
    let readonly = readonly_protected_artifacts().join(", ");
    let append_only = append_only_protected_artifacts().join(", ");
    format!(
        "⚠ ORCHESTRATOR-OWNED ARTIFACT — DO NOT EDIT: {readonly} are generated from the typed event stream. {append_only} is append-only via typed tools. Postflight reverts unauthorized edits and emits an UnauthorizedArtifactEdit event."
    )
}
