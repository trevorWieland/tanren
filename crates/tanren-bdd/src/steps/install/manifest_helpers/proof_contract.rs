use std::path::Path;

use tanren_testkit::{
    assert_manifest_rust_cargo_defaults as contract_assert_manifest_rust_cargo_defaults,
    assert_rust_cargo_default_assets_installed as contract_assert_rust_cargo_default_assets_installed,
    assert_rust_cargo_standards_installed as contract_assert_rust_cargo_standards_installed,
    assert_selected_integration_command_assets as contract_assert_selected_integration_command_assets,
    assert_uninstall_preserves_baseline_file_content as contract_assert_uninstall_preserves_baseline_file_content,
    assert_uninstall_removes_generated_assets_and_manifest as contract_assert_uninstall_removes_generated_assets_and_manifest,
};

use super::RepositoryRelativePath;
use crate::steps::install::InstallStepError;

pub(crate) fn assert_rust_cargo_default_assets_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract_assert_rust_cargo_default_assets_installed(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_rust_cargo_standards_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract_assert_rust_cargo_standards_installed(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_selected_integration_command_assets(
    repository_root: &Path,
    selected_integrations: &str,
) -> Result<(), InstallStepError> {
    contract_assert_selected_integration_command_assets(repository_root, selected_integrations)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_manifest_rust_cargo_defaults(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract_assert_manifest_rust_cargo_defaults(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_uninstall_removes_generated_assets_and_manifest(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract_assert_uninstall_removes_generated_assets_and_manifest(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_uninstall_preserves_baseline_file_content(
    repository_root: &Path,
    relative_path: &RepositoryRelativePath,
    baseline: &[u8],
) -> Result<(), InstallStepError> {
    contract_assert_uninstall_preserves_baseline_file_content(
        repository_root,
        &relative_path.as_install_path()?,
        baseline,
    )
    .map_err(|source| InstallStepError::InstallProofFailure { source })
}
