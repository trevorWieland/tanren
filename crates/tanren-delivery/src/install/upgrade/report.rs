//! Upgrade preview report formatting and structured output encoding.

use crate::install::InstallPlan;
use crate::install::manifest::{ManifestVersion, RepoRelativePath};
use crate::install::plan::PlannedWriteKind;

/// Typed upgrade compatibility concern emitted in previews.
///
/// Each variant carries the data that motivated the concern. No synthetic
/// `None` variant exists — an empty concern list signals no concerns.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum UpgradeCompatibilityConcern {
    /// Repository has no prior Tanren install manifest.
    NoInstallManifest,
    /// One or more generated assets will be replaced or removed.
    DestructiveAssetChanges(Vec<RepoRelativePath>),
    /// Manifest version is not supported for migration.
    UnsupportedManifestVersion {
        detected: ManifestVersion,
        min_supported: ManifestVersion,
        current: ManifestVersion,
    },
}

impl UpgradeCompatibilityConcern {
    /// Stable concern code rendered in CLI output.
    #[must_use]
    pub fn as_code(&self) -> &'static str {
        match self {
            Self::NoInstallManifest => "no-install-manifest",
            Self::DestructiveAssetChanges(_) => "destructive-asset-changes",
            Self::UnsupportedManifestVersion { .. } => "unsupported-manifest-version",
        }
    }
}

/// Renderable upgrade preview details.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct UpgradePreviewReport {
    changed: Vec<RepoRelativePath>,
    destructive: Vec<RepoRelativePath>,
    restored: Vec<RepoRelativePath>,
    removed: Vec<RepoRelativePath>,
    preserved: Vec<RepoRelativePath>,
    compatibility_concerns: Vec<UpgradeCompatibilityConcern>,
}

impl UpgradePreviewReport {
    /// Preview report for repositories without an install manifest.
    #[must_use]
    pub fn no_install_manifest() -> Self {
        Self {
            changed: Vec::new(),
            destructive: Vec::new(),
            restored: Vec::new(),
            removed: Vec::new(),
            preserved: Vec::new(),
            compatibility_concerns: vec![UpgradeCompatibilityConcern::NoInstallManifest],
        }
    }

    /// Preview report for manifest versions that cannot be migrated.
    #[must_use]
    pub fn unsupported_manifest_version(
        detected: ManifestVersion,
        min_supported: ManifestVersion,
        current: ManifestVersion,
    ) -> Self {
        Self {
            changed: Vec::new(),
            destructive: Vec::new(),
            restored: Vec::new(),
            removed: Vec::new(),
            preserved: Vec::new(),
            compatibility_concerns: vec![UpgradeCompatibilityConcern::UnsupportedManifestVersion {
                detected,
                min_supported,
                current,
            }],
        }
    }

    /// Build a preview report from a validated install plan.
    #[must_use]
    pub fn from_plan(plan: &InstallPlan) -> Self {
        let mut changed = plan
            .writes()
            .iter()
            .map(|write| write.path().clone())
            .collect::<Vec<_>>();
        changed.extend(plan.removals().iter().map(|removal| removal.path().clone()));
        changed.push(plan.manifest_path().clone());
        changed.sort();
        changed.dedup();

        let mut destructive = plan
            .writes()
            .iter()
            .filter(|write| {
                matches!(
                    write.kind(),
                    PlannedWriteKind::Updated | PlannedWriteKind::Restored
                )
            })
            .map(|write| write.path().clone())
            .collect::<Vec<_>>();
        destructive.extend(plan.removals().iter().map(|removal| removal.path().clone()));
        destructive.sort();
        destructive.dedup();

        let restored = plan
            .writes()
            .iter()
            .filter(|write| matches!(write.kind(), PlannedWriteKind::Restored))
            .map(|write| write.path().clone())
            .collect();

        let removed = plan
            .removals()
            .iter()
            .map(|removal| removal.path().clone())
            .collect();

        let mut preserved = plan.preserved().to_vec();
        preserved.sort();
        preserved.dedup();

        let compatibility_concerns = if destructive.is_empty() {
            Vec::new()
        } else {
            vec![UpgradeCompatibilityConcern::DestructiveAssetChanges(
                destructive.clone(),
            )]
        };

        Self {
            changed,
            destructive,
            restored,
            removed,
            preserved,
            compatibility_concerns,
        }
    }

    /// Changed repository paths in deterministic order.
    #[must_use]
    pub fn changed(&self) -> &[RepoRelativePath] {
        &self.changed
    }

    /// Paths subject to destructive operations in deterministic order.
    #[must_use]
    pub fn destructive(&self) -> &[RepoRelativePath] {
        &self.destructive
    }

    /// Restored paths in deterministic order.
    #[must_use]
    pub fn restored(&self) -> &[RepoRelativePath] {
        &self.restored
    }

    /// Removed paths in deterministic order.
    #[must_use]
    pub fn removed(&self) -> &[RepoRelativePath] {
        &self.removed
    }

    /// Preserved paths in deterministic order.
    #[must_use]
    pub fn preserved(&self) -> &[RepoRelativePath] {
        &self.preserved
    }

    /// Compatibility and migration concern entries.
    #[must_use]
    pub fn compatibility_concerns(&self) -> &[UpgradeCompatibilityConcern] {
        &self.compatibility_concerns
    }
}

/// Safely encode a field value for machine-readable key=value output.
///
/// Replaces newlines, carriage returns, tabs, and commas with escaped
/// equivalents so a single `key=value` token remains parseable without
/// ambiguity. Paths and labels containing control characters or delimiters
/// are represented safely.
#[must_use]
pub fn encode_field(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ',' => out.push_str("\\,"),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out
}

/// Join sorted repository-relative paths with a safe encoding for
/// machine-readable bracket fields.
#[must_use]
pub fn format_encoded_path_list(paths: &[RepoRelativePath]) -> String {
    paths
        .iter()
        .map(|p| encode_field(p.as_str()))
        .collect::<Vec<_>>()
        .join(",")
}
