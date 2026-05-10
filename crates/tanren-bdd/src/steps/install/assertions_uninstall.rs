use super::context::InstallContext;
use super::manifest_helpers;
use super::manifest_helpers::RepositoryRelativePath;
use super::{InstallStepError, InstallStepResult};

impl InstallContext {
    pub(crate) fn assert_uninstall_preview_leaves_repository_snapshot_unchanged(
        &self,
    ) -> InstallStepResult<()> {
        let before = self
            .uninstall_snapshot_before_last_run
            .as_ref()
            .ok_or(InstallStepError::MissingSnapshotBeforeRun)?;
        manifest_helpers::assert_uninstall_preview_keeps_repository_snapshot_unchanged(
            &self.repository_root,
            before,
        )
    }

    pub(crate) fn assert_uninstall_no_install_leaves_repository_snapshot_unchanged(
        &self,
    ) -> InstallStepResult<()> {
        let before = self
            .uninstall_snapshot_before_last_run
            .as_ref()
            .ok_or(InstallStepError::MissingSnapshotBeforeRun)?;
        manifest_helpers::assert_uninstall_no_install_keeps_repository_snapshot_unchanged(
            &self.repository_root,
            before,
        )
    }

    pub(crate) fn assert_uninstall_preserves_baseline_file_content(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        if relative_path.as_str().starts_with("profiles/rust-cargo/") {
            return self.assert_uninstall_preserves_standards_baseline_file_content(relative_path);
        }
        if relative_path.as_str().starts_with("docs/behaviors/") {
            return self.assert_uninstall_preserves_spec_baseline_file_content(relative_path);
        }
        if relative_path.as_str().starts_with("crates/") {
            return self
                .assert_uninstall_preserves_source_signal_baseline_file_content(relative_path);
        }
        self.assert_uninstall_preserves_drifted_generated_baseline_file_content(relative_path)
    }

    pub(crate) fn assert_uninstall_preserves_standards_baseline_file_content(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let baseline =
            self.baselines
                .get(relative_path)
                .ok_or_else(|| InstallStepError::MissingBaseline {
                    path: relative_path.as_str().to_owned(),
                })?;
        manifest_helpers::assert_uninstall_preserves_standards_baseline_file_content(
            &self.repository_root,
            relative_path,
            baseline,
        )
    }

    pub(crate) fn assert_uninstall_preserves_spec_baseline_file_content(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let baseline =
            self.baselines
                .get(relative_path)
                .ok_or_else(|| InstallStepError::MissingBaseline {
                    path: relative_path.as_str().to_owned(),
                })?;
        manifest_helpers::assert_uninstall_preserves_spec_baseline_file_content(
            &self.repository_root,
            relative_path,
            baseline,
        )
    }

    pub(crate) fn assert_uninstall_preserves_source_signal_baseline_file_content(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let baseline =
            self.baselines
                .get(relative_path)
                .ok_or_else(|| InstallStepError::MissingBaseline {
                    path: relative_path.as_str().to_owned(),
                })?;
        manifest_helpers::assert_uninstall_preserves_source_signal_baseline_file_content(
            &self.repository_root,
            relative_path,
            baseline,
        )
    }

    pub(crate) fn assert_uninstall_preserves_drifted_generated_baseline_file_content(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let baseline =
            self.baselines
                .get(relative_path)
                .ok_or_else(|| InstallStepError::MissingBaseline {
                    path: relative_path.as_str().to_owned(),
                })?;
        manifest_helpers::assert_uninstall_preserves_drifted_generated_baseline_file_content(
            &self.repository_root,
            relative_path,
            baseline,
        )
    }
}
