//! Upgrade preview report formatting.

use crate::install::InstallPlan;
use crate::install::manifest::RepoRelativePath;
use crate::install::plan::PlannedWriteKind;

/// Typed upgrade compatibility concern emitted in previews.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UpgradeCompatibilityConcern {
    NoInstallManifest,
    None,
    DestructiveAssetChanges,
}

impl UpgradeCompatibilityConcern {
    /// Stable concern code rendered in CLI output.
    #[must_use]
    pub(super) const fn as_code(self) -> &'static str {
        match self {
            Self::NoInstallManifest => "no-install-manifest",
            Self::None => "none",
            Self::DestructiveAssetChanges => "destructive-asset-changes",
        }
    }
}

/// Typed view of preview report sections for CLI rendering.
#[derive(Debug, Clone, Copy)]
pub(super) struct UpgradePreviewRender<'a> {
    changed_paths: &'a [RepoRelativePath],
    destructive_actions: &'a [RepoRelativePath],
    preserved_paths: &'a [RepoRelativePath],
    compatibility_concerns: &'a [UpgradeCompatibilityConcern],
}

impl UpgradePreviewRender<'_> {
    /// Changed path count.
    #[must_use]
    pub(super) fn changed_count(&self) -> usize {
        self.changed_paths.len()
    }

    /// Destructive action count.
    #[must_use]
    pub(super) fn destructive_count(&self) -> usize {
        self.destructive_actions.len()
    }

    /// Preserved path count.
    #[must_use]
    pub(super) fn preserved_count(&self) -> usize {
        self.preserved_paths.len()
    }

    /// Compatibility concern count.
    #[must_use]
    pub(super) fn concern_count(&self) -> usize {
        self.compatibility_concerns.len()
    }

    /// Comma-joined changed path list for stable CLI output.
    #[must_use]
    pub(super) fn changed_paths_csv(&self) -> String {
        format_path_list(self.changed_paths)
    }

    /// Comma-joined destructive path list for stable CLI output.
    #[must_use]
    pub(super) fn destructive_paths_csv(&self) -> String {
        format_path_list(self.destructive_actions)
    }

    /// Comma-joined preserved path list for stable CLI output.
    #[must_use]
    pub(super) fn preserved_paths_csv(&self) -> String {
        format_path_list(self.preserved_paths)
    }

    /// Comma-joined concern code list for stable CLI output.
    #[must_use]
    pub(super) fn concern_codes_csv(&self) -> String {
        self.compatibility_concerns
            .iter()
            .map(|concern| concern.as_code())
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Renderable upgrade preview details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UpgradePreviewReport {
    changed_paths: Vec<RepoRelativePath>,
    destructive_actions: Vec<RepoRelativePath>,
    preserved_paths: Vec<RepoRelativePath>,
    compatibility_concerns: Vec<UpgradeCompatibilityConcern>,
}

impl UpgradePreviewReport {
    /// Preview report for repositories without an install manifest.
    #[must_use]
    pub(super) fn no_install_manifest() -> Self {
        Self {
            changed_paths: Vec::new(),
            destructive_actions: Vec::new(),
            preserved_paths: Vec::new(),
            compatibility_concerns: vec![UpgradeCompatibilityConcern::NoInstallManifest],
        }
    }

    /// Build a preview report from a validated install plan.
    #[must_use]
    pub(super) fn from_plan(plan: &InstallPlan) -> Self {
        let mut changed_paths = plan
            .writes()
            .iter()
            .map(|write| write.path().clone())
            .collect::<Vec<_>>();
        changed_paths.extend(plan.removals().iter().map(|removal| removal.path().clone()));
        changed_paths.push(plan.manifest_path().clone());
        changed_paths.sort();
        changed_paths.dedup();

        let mut destructive_actions = plan
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
        destructive_actions.extend(plan.removals().iter().map(|removal| removal.path().clone()));
        destructive_actions.sort();
        destructive_actions.dedup();

        let mut preserved_paths = plan.preserved().to_vec();
        preserved_paths.sort();
        preserved_paths.dedup();

        let compatibility_concerns = if destructive_actions.is_empty() {
            vec![UpgradeCompatibilityConcern::None]
        } else {
            vec![UpgradeCompatibilityConcern::DestructiveAssetChanges]
        };

        Self {
            changed_paths,
            destructive_actions,
            preserved_paths,
            compatibility_concerns,
        }
    }

    /// Changed repository paths in deterministic order.
    #[must_use]
    pub(super) fn changed_paths(&self) -> &[RepoRelativePath] {
        &self.changed_paths
    }

    /// Paths subject to destructive operations in deterministic order.
    #[must_use]
    pub(super) fn destructive_actions(&self) -> &[RepoRelativePath] {
        &self.destructive_actions
    }

    /// Preserved paths in deterministic order.
    #[must_use]
    pub(super) fn preserved_paths(&self) -> &[RepoRelativePath] {
        &self.preserved_paths
    }

    /// Compatibility and migration concern codes.
    #[must_use]
    pub(super) fn compatibility_concerns(&self) -> &[UpgradeCompatibilityConcern] {
        &self.compatibility_concerns
    }

    /// Render-focused typed view consumed by CLI formatting.
    #[must_use]
    pub(super) fn render(&self) -> UpgradePreviewRender<'_> {
        UpgradePreviewRender {
            changed_paths: self.changed_paths(),
            destructive_actions: self.destructive_actions(),
            preserved_paths: self.preserved_paths(),
            compatibility_concerns: self.compatibility_concerns(),
        }
    }
}

/// Join sorted repository-relative paths for stable CLI output.
#[must_use]
fn format_path_list(paths: &[RepoRelativePath]) -> String {
    paths
        .iter()
        .map(RepoRelativePath::as_str)
        .collect::<Vec<_>>()
        .join(",")
}
