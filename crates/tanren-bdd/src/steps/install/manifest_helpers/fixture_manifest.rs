use std::path::Path;

use tanren_cli_app::install::contract;

use crate::steps::install::InstallStepError;

use super::RepositoryRelativePath;

pub(crate) fn append_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &RepositoryRelativePath,
    content_hash: &str,
) -> Result<(), InstallStepError> {
    contract::append_stale_generated_manifest_entry(
        manifest,
        &relative_path.as_install_path()?,
        content_hash,
    );
    Ok(())
}

pub(crate) fn tamper_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallStepError> {
    contract::tamper_manifest_with_raw_generated_entry(repository_root, raw_path)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}
