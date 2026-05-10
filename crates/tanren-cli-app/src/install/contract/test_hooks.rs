use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use crate::install::InstallIntegration;
use crate::install::manifest::{INSTALL_MANIFEST_REPO_PATH, RepoRelativePath};

use super::InstallProofError;

const TAMPERED_ENTRY_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Append a stale generated-manifest row for mutation-flow fixtures.
#[cfg(feature = "test-hooks")]
pub fn append_uninstall_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &RepoRelativePath,
    content_hash: &str,
) {
    let _ = write!(
        manifest,
        "\n[[entries]]\npath = \"{}\"\ncontent_hash = \"{}\"\nasset_class = \"methodology-command\"\nintegration = \"{}\"\npreservation = \"replace-generated\"\n",
        relative_path.as_str(),
        content_hash,
        InstallIntegration::Codex.as_str()
    );
}

/// Inject a raw stale generated-manifest row (used by traversal tamper witnesses).
#[cfg(feature = "test-hooks")]
pub fn tamper_uninstall_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallProofError> {
    let manifest_path = repository_root.join(INSTALL_MANIFEST_REPO_PATH);
    let mut manifest = read_to_string_with_context(&manifest_path, "read install manifest")?;
    let path_line = format!("path = \"{raw_path}\"");
    if manifest.contains(&path_line) {
        return Err(InstallProofError::StaleManifestPathAlreadyPresent {
            path: raw_path.to_owned(),
        });
    }
    let _ = write!(
        manifest,
        "\n[[entries]]\npath = \"{raw_path}\"\ncontent_hash = \"{TAMPERED_ENTRY_SHA256}\"\nasset_class = \"methodology-command\"\nintegration = \"{}\"\npreservation = \"replace-generated\"\n",
        InstallIntegration::Codex.as_str()
    );
    fs::write(&manifest_path, manifest).map_err(|source| InstallProofError::WriteFile {
        path: manifest_path,
        action: "write install manifest with tampered raw entry",
        source,
    })?;
    Ok(())
}

/// Read a workspace catalog file for fixture seeding.
#[cfg(feature = "test-hooks")]
pub fn read_workspace_catalog_file(
    relative_path: &RepoRelativePath,
) -> Result<String, InstallProofError> {
    let absolute = workspace_root()?.join(relative_path.as_str());
    read_to_string_with_context(&absolute, "read workspace catalog file")
}

fn workspace_root() -> Result<PathBuf, InstallProofError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|source| InstallProofError::CanonicalizeWorkspaceRoot {
            action: "resolving workspace root",
            source,
        })
}

fn read_to_string_with_context(
    path: &Path,
    action: &'static str,
) -> Result<String, InstallProofError> {
    fs::read_to_string(path).map_err(|source| InstallProofError::ReadFile {
        path: path.to_path_buf(),
        action,
        source,
    })
}
