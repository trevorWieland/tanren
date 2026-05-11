use tanren_testkit::read_workspace_catalog_file as contract_read_workspace_catalog_file;

use crate::steps::install::InstallStepError;

use super::RepositoryRelativePath;

pub(crate) fn read_workspace_catalog_file(relative_path: &str) -> Result<String, InstallStepError> {
    let parsed = RepositoryRelativePath::parse(relative_path.to_owned())?.as_install_path();
    contract_read_workspace_catalog_file(&parsed)
        .map_err(|source| InstallStepError::InstallProofFailure { source })
}
