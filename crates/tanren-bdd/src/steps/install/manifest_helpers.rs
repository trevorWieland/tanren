use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use super::InstallStepError;

const NIBBLES: &[u8; 16] = b"0123456789abcdef";
const TAMPERED_ENTRY_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

pub(crate) fn assert_rust_cargo_default_assets_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    let command_files = list_relative_files_under_workspace("commands/project")?;
    if command_files.is_empty() {
        return Err(InstallStepError::CatalogEmpty {
            catalog_root: "commands/project",
        });
    }
    for command_file in &command_files {
        for integration_destination in [
            format!(".claude/commands/{command_file}"),
            format!(".codex/skills/{command_file}"),
            format!(".opencode/commands/{command_file}"),
        ] {
            assert_file_exists(repository_root, &integration_destination)?;
        }
    }

    let standards_files = list_relative_files_under_workspace("profiles/rust-cargo")?;
    if standards_files.is_empty() {
        return Err(InstallStepError::CatalogEmpty {
            catalog_root: "profiles/rust-cargo",
        });
    }
    for standard_file in standards_files {
        assert_file_exists(
            repository_root,
            &format!("profiles/rust-cargo/{standard_file}"),
        )?;
    }

    Ok(())
}

pub(crate) fn assert_rust_cargo_standards_installed(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    let standards_files = list_relative_files_under_workspace("profiles/rust-cargo")?;
    if standards_files.is_empty() {
        return Err(InstallStepError::CatalogEmpty {
            catalog_root: "profiles/rust-cargo",
        });
    }
    for standard_file in standards_files {
        assert_file_exists(
            repository_root,
            &format!("profiles/rust-cargo/{standard_file}"),
        )?;
    }
    Ok(())
}

pub(crate) fn assert_selected_integration_command_assets(
    repository_root: &Path,
    selected_integrations: &[String],
) -> Result<(), InstallStepError> {
    let mut selected = BTreeSet::new();
    for integration in selected_integrations {
        let normalized = normalize_integration_name(integration.as_str()).ok_or_else(|| {
            InstallStepError::UnsupportedIntegrationForAssertion {
                integration: integration.clone(),
            }
        })?;
        selected.insert(normalized);
    }

    let command_files = list_relative_files_under_workspace("commands/project")?;
    if command_files.is_empty() {
        return Err(InstallStepError::CatalogEmpty {
            catalog_root: "commands/project",
        });
    }

    for command_file in &command_files {
        for (integration, destination) in [
            ("claude", format!(".claude/commands/{command_file}")),
            ("codex", format!(".codex/skills/{command_file}")),
            ("open-code", format!(".opencode/commands/{command_file}")),
        ] {
            if selected.contains(integration) {
                assert_file_exists(repository_root, &destination)?;
            } else {
                assert_file_absent(repository_root, &destination)?;
            }
        }
    }

    Ok(())
}

pub(crate) fn assert_manifest_rust_cargo_defaults(
    repository_root: &Path,
) -> Result<(), InstallStepError> {
    let manifest_path = repository_root.join(".tanren/install-manifest.toml");
    let manifest = read_to_string_with_context(
        &manifest_path,
        "read install manifest from repository fixture",
    )?;
    for expected in [
        "manifest_version = 1",
        "profile = \"rust-cargo\"",
        "integrations = [\"claude\", \"codex\", \"open-code\"]",
        "asset_class = \"methodology-command\"",
        "asset_class = \"standards-profile\"",
    ] {
        if !manifest.contains(expected) {
            return Err(InstallStepError::ManifestMissingContent {
                expected: expected.to_owned(),
                manifest_path,
                manifest,
            });
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RepositoryRelativePath(String);

impl RepositoryRelativePath {
    pub(crate) fn parse(raw: String) -> Result<Self, InstallStepError> {
        validate_relative_path(raw.as_str())?;
        Ok(Self(raw))
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

pub(crate) fn validate_relative_path(path: &str) -> Result<(), InstallStepError> {
    if path.is_empty() {
        return Err(InstallStepError::EmptyRepositoryRelativePath);
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(InstallStepError::AbsoluteRepositoryRelativePath {
            path: path.to_owned(),
        });
    }
    let is_valid = candidate
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir));
    if !is_valid {
        return Err(InstallStepError::TraversalRepositoryRelativePath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn append_stale_generated_manifest_entry(
    manifest: &mut String,
    relative_path: &RepositoryRelativePath,
    content_hash: String,
) {
    let entry =
        GeneratedManifestEntry::stale_methodology_command(relative_path.clone(), content_hash);
    manifest.push_str(&entry.to_manifest_block());
}

pub(crate) fn tamper_manifest_with_raw_generated_entry(
    repository_root: &Path,
    raw_path: &str,
) -> Result<(), InstallStepError> {
    let manifest_path = repository_root.join(".tanren/install-manifest.toml");
    let mut manifest = read_to_string_with_context(&manifest_path, "read install manifest")?;
    let path_line = format!("path = \"{raw_path}\"");
    if manifest.contains(&path_line) {
        return Err(InstallStepError::StaleManifestPathAlreadyPresent {
            path: raw_path.to_owned(),
        });
    }
    let _ = write!(
        manifest,
        "\n[[entries]]\npath = \"{raw_path}\"\ncontent_hash = \"{TAMPERED_ENTRY_SHA256}\"\nasset_class = \"methodology-command\"\nintegration = \"codex\"\npreservation = \"replace-generated\"\n"
    );
    fs::write(&manifest_path, manifest).map_err(|source| InstallStepError::WriteFile {
        path: manifest_path,
        action: "write install manifest with tampered raw entry",
        source,
    })?;
    Ok(())
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push(char::from(NIBBLES[(byte >> 4) as usize]));
        hex.push(char::from(NIBBLES[(byte & 0x0f) as usize]));
    }
    hex
}

pub(crate) fn read_workspace_catalog_file(relative_path: &str) -> Result<String, InstallStepError> {
    validate_relative_path(relative_path)?;
    let absolute = workspace_root()?.join(relative_path);
    read_to_string_with_context(&absolute, "read workspace catalog file")
}

fn assert_file_exists(repository_root: &Path, relative_path: &str) -> Result<(), InstallStepError> {
    let absolute = repository_root.join(relative_path);
    if !absolute.exists() {
        return Err(InstallStepError::ExpectedFileToExist { path: absolute });
    }
    Ok(())
}

fn assert_file_absent(repository_root: &Path, relative_path: &str) -> Result<(), InstallStepError> {
    let absolute = repository_root.join(relative_path);
    if absolute.exists() {
        return Err(InstallStepError::ExpectedFileToBeAbsent { path: absolute });
    }
    Ok(())
}

fn normalize_integration_name(raw: &str) -> Option<&'static str> {
    match raw {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        "opencode" | "open-code" => Some("open-code"),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedManifestEntry {
    path: RepositoryRelativePath,
    content_hash: String,
    asset_class: &'static str,
    integration: &'static str,
    preservation: &'static str,
}

impl GeneratedManifestEntry {
    fn stale_methodology_command(path: RepositoryRelativePath, content_hash: String) -> Self {
        Self {
            path,
            content_hash,
            asset_class: "methodology-command",
            integration: "codex",
            preservation: "replace-generated",
        }
    }

    fn to_manifest_block(&self) -> String {
        format!(
            "\n[[entries]]\npath = \"{}\"\ncontent_hash = \"{}\"\nasset_class = \"{}\"\nintegration = \"{}\"\npreservation = \"{}\"\n",
            self.path.as_str(),
            self.content_hash,
            self.asset_class,
            self.integration,
            self.preservation
        )
    }
}

fn list_relative_files_under_workspace(
    relative_root: &'static str,
) -> Result<Vec<String>, InstallStepError> {
    let root = workspace_root()?.join(relative_root);
    if !root.exists() || !root.is_dir() {
        return Err(InstallStepError::MissingCatalogRoot { path: root });
    }

    let mut files = BTreeMap::new();
    collect_files(&root, &root, &mut files)?;
    Ok(files.into_keys().collect())
}

fn workspace_root() -> Result<PathBuf, InstallStepError> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .map_err(|source| InstallStepError::CanonicalizeWorkspaceRoot { source })
}

fn collect_files(
    root: &Path,
    cursor: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), InstallStepError> {
    let entries = fs::read_dir(cursor).map_err(|source| InstallStepError::ReadDirectory {
        path: cursor.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| InstallStepError::ReadDirectoryEntry {
            path: cursor.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| InstallStepError::InspectFileType {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_files(root, &path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative =
            path.strip_prefix(root)
                .map_err(|source| InstallStepError::PathOutsideRoot {
                    path: path.clone(),
                    root: root.to_path_buf(),
                    source,
                })?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        let bytes = read_bytes_with_context(&path, "read workspace catalog file")?;
        out.insert(relative, bytes);
    }
    Ok(())
}

fn read_to_string_with_context(
    path: &Path,
    action: &'static str,
) -> Result<String, InstallStepError> {
    fs::read_to_string(path).map_err(|source| InstallStepError::ReadFile {
        path: path.to_path_buf(),
        action,
        source,
    })
}

fn read_bytes_with_context(path: &Path, action: &'static str) -> Result<Vec<u8>, InstallStepError> {
    fs::read(path).map_err(|source| InstallStepError::ReadFile {
        path: path.to_path_buf(),
        action,
        source,
    })
}

pub(crate) fn io_error(
    path: PathBuf,
    action: &'static str,
    source: std::io::Error,
) -> InstallStepError {
    InstallStepError::Io {
        path,
        action,
        source,
    }
}
