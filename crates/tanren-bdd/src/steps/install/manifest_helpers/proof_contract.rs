use std::path::Path;

use tanren_cli_app::install::contract;

use crate::steps::install::InstallStepError;

pub(crate) fn assert_rust_cargo_default_assets_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract::assert_rust_cargo_default_assets_installed(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_rust_cargo_standards_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract::assert_rust_cargo_standards_installed(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_selected_integration_command_assets(
    repository_root: &Path,
    selected_integrations: &str,
) -> Result<(), InstallStepError> {
    contract::assert_selected_integration_command_assets(repository_root, selected_integrations)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn assert_manifest_rust_cargo_defaults(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    contract::assert_manifest_rust_cargo_defaults(repository_root)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}
