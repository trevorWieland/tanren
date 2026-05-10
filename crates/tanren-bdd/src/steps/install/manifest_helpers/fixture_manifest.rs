use std::path::Path;

use tanren_testkit::{
    append_stale_generated_manifest_entry as contract_append_stale_generated_manifest_entry,
    tamper_manifest_with_raw_generated_entry as contract_tamper_manifest_with_raw_generated_entry,
};

use crate::steps::install::InstallStepError;

use super::RepositoryRelativePath;

pub(crate) fn append_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &RepositoryRelativePath,
    content_hash: &str,
) -> Result<(), InstallStepError> {
    contract_append_stale_generated_manifest_entry(
        manifest,
        &relative_path.as_install_path()?,
        content_hash,
    )
    .map_err(|source| InstallStepError::InstallProofFailure { source })
}

pub(crate) fn tamper_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallStepError> {
    contract_tamper_manifest_with_raw_generated_entry(repository_root, raw_path)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}
