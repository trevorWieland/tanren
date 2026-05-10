use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use axum::http::StatusCode;
use serde::Serialize;
use tanren_delivery::install::{InstallManifest, InstallProfile, cached_catalog_destination_paths};

const INSTALL_MANIFEST_PATH: &str = ".tanren/install-manifest.toml";
const TANREN_METADATA_ROOT: &str = ".tanren";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct RepositorySnapshot {
    pub(super) directories: Vec<String>,
    pub(super) files: Vec<(String, String)>,
    pub(super) symlinks: Vec<(String, String)>,
}

pub(super) fn capture_scoped_snapshot(
    repository_root: &Path,
    workspace_root: &Path,
    explicit_paths: &BTreeSet<String>,
) -> Result<RepositorySnapshot, (StatusCode, String)> {
    let mut exact_paths = load_catalog_generated_paths(workspace_root);
    exact_paths.extend(load_manifest_generated_paths(repository_root)?);
    exact_paths.extend(explicit_paths.iter().cloned());
    exact_paths.insert(INSTALL_MANIFEST_PATH.to_owned());

    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut symlinks = Vec::new();
    let mut visited = BTreeSet::new();
    for tracked_path in exact_paths {
        capture_tracked_path(
            repository_root,
            tracked_path.as_str(),
            &mut directories,
            &mut files,
            &mut symlinks,
            &mut visited,
        )?;
    }
    capture_tracked_path(
        repository_root,
        TANREN_METADATA_ROOT,
        &mut directories,
        &mut files,
        &mut symlinks,
        &mut visited,
    )?;

    directories.sort();
    files.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    symlinks.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

    Ok(RepositorySnapshot {
        directories,
        files,
        symlinks,
    })
}

fn capture_tracked_path(
    root: &Path,
    tracked_path: &str,
    directories: &mut Vec<String>,
    files: &mut Vec<(String, String)>,
    symlinks: &mut Vec<(String, String)>,
    visited: &mut BTreeSet<String>,
) -> Result<(), (StatusCode, String)> {
    let Some(relative_path) = normalize_relative_path(tracked_path) else {
        return Ok(());
    };
    if visited.contains(relative_path.as_str()) {
        return Ok(());
    }
    visited.insert(relative_path.clone());

    let absolute = root.join(relative_path.as_str());
    let metadata = match fs::symlink_metadata(&absolute) {
        Ok(value) => value,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(io_error(
                &source,
                &absolute,
                "inspect repository fixture path",
            ));
        }
    };

    if metadata.is_dir() {
        directories.push(relative_path.clone());
        let mut entries = fs::read_dir(&absolute)
            .map_err(|source| io_error(&source, &absolute, "read repository fixture directory"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| {
                io_error(
                    &source,
                    &absolute,
                    "read repository fixture directory entry",
                )
            })?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let child_name = entry.file_name().to_string_lossy().into_owned();
            let child_path = format!("{relative_path}/{child_name}");
            capture_tracked_path(
                root,
                child_path.as_str(),
                directories,
                files,
                symlinks,
                visited,
            )?;
        }
        return Ok(());
    }

    if metadata.file_type().is_symlink() {
        let target = fs::read_link(&absolute).map_err(|source| {
            io_error(&source, &absolute, "read repository fixture symlink target")
        })?;
        let normalized_target = target.to_string_lossy().replace('\\', "/");
        symlinks.push((relative_path, normalized_target));
        return Ok(());
    }

    if !metadata.is_file() {
        return Ok(());
    }

    let hash = tanren_delivery::install::sha256_hex_file(&absolute)
        .map_err(|source| io_error(&source, &absolute, "hash repository fixture file"))?;
    files.push((relative_path, hash.to_string()));
    Ok(())
}

fn load_manifest_generated_paths(
    repository_root: &Path,
) -> Result<BTreeSet<String>, (StatusCode, String)> {
    let manifest_path = repository_root.join(INSTALL_MANIFEST_PATH);
    let raw = match fs::read_to_string(&manifest_path) {
        Ok(raw) => raw,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeSet::new()),
        Err(source) => return Err(io_error(&source, &manifest_path, "read install manifest")),
    };
    let manifest = InstallManifest::parse_toml(&raw).map_err(|source| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to parse install manifest: {source}"),
        )
    })?;
    let mut paths = BTreeSet::new();
    for entry in &manifest.entries {
        if let Some(normalized) = normalize_relative_path(entry.path.as_str()) {
            paths.insert(normalized);
        }
    }
    Ok(paths)
}

fn load_catalog_generated_paths(_workspace_root: &Path) -> BTreeSet<String> {
    let mut generated_paths = BTreeSet::new();
    for path in cached_catalog_destination_paths(InstallProfile::RustCargo) {
        if let Some(normalized) = normalize_relative_path(path) {
            generated_paths.insert(normalized);
        }
    }
    generated_paths
}

fn normalize_relative_path(value: &str) -> Option<String> {
    let trimmed = value.trim().replace('\\', "/");
    if trimmed.is_empty() || trimmed == "." || trimmed.starts_with('/') {
        return None;
    }
    let normalized = trimmed.trim_start_matches("./").trim_end_matches('/');
    if normalized.is_empty() {
        return None;
    }
    let is_valid = normalized
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..");
    if !is_valid {
        return None;
    }
    Some(normalized.to_owned())
}

fn io_error(source: &std::io::Error, path: &Path, action: &'static str) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("failed to {action} at '{}': {source}", path.display()),
    )
}
