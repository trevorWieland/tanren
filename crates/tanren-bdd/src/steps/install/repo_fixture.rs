use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::context::InstallContext;
use super::manifest_helpers;
use super::manifest_helpers::RepositoryRelativePath;
use super::{InstallStepError, InstallStepResult};

static SCENARIO_COUNTER: AtomicU64 = AtomicU64::new(0);

impl InstallContext {
    pub(crate) fn write_fixture_file(
        &mut self,
        relative_path: &RepositoryRelativePath,
        content: String,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                manifest_helpers::io_error(
                    parent.to_path_buf(),
                    "create parent directories in repository fixture",
                    source,
                )
            })?;
        }
        fs::write(&absolute, content).map_err(|source| InstallStepError::WriteFile {
            path: absolute,
            action: "write repository fixture file",
            source,
        })?;
        Ok(())
    }

    pub(crate) fn record_baseline(
        &mut self,
        relative_path: RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        let bytes = fs::read(&absolute).map_err(|source| InstallStepError::ReadFile {
            path: absolute,
            action: "read repository fixture file for baseline",
            source,
        })?;
        self.baselines.insert(relative_path, bytes);
        Ok(())
    }

    pub(crate) fn inject_manifest_stale_generated_entry(
        &mut self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        self.assert_file_exists(relative_path)?;

        let manifest_path = self.repository_path(".tanren/install-manifest.toml")?;
        let mut manifest =
            fs::read_to_string(&manifest_path).map_err(|source| InstallStepError::ReadFile {
                path: manifest_path.clone(),
                action: "read install manifest",
                source,
            })?;
        let path_line = format!("path = \"{}\"", relative_path.as_str());
        if manifest.contains(&path_line) {
            return Err(InstallStepError::StaleManifestPathAlreadyPresent {
                path: relative_path.as_str().to_owned(),
            });
        }
        let stale_path = self.repository_path(relative_path.as_str())?;
        let stale_bytes = fs::read(&stale_path).map_err(|source| InstallStepError::ReadFile {
            path: stale_path,
            action: "read stale generated file for manifest hash",
            source,
        })?;
        manifest_helpers::append_stale_generated_manifest_entry(
            &mut manifest,
            relative_path,
            manifest_helpers::sha256_hex(&stale_bytes),
        );
        fs::write(&manifest_path, manifest).map_err(|source| InstallStepError::WriteFile {
            path: manifest_path,
            action: "write install manifest with stale entry",
            source,
        })?;
        Ok(())
    }

    pub(crate) fn inject_manifest_raw_generated_entry_path(
        &mut self,
        raw_path: &str,
    ) -> InstallStepResult<()> {
        manifest_helpers::tamper_manifest_with_raw_generated_entry(&self.repository_root, raw_path)
    }

    pub(crate) fn delete_fixture_file(
        &mut self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        if !absolute.exists() {
            return Err(InstallStepError::ExpectedFileToExist { path: absolute });
        }
        fs::remove_file(&absolute).map_err(|source| InstallStepError::Io {
            path: absolute,
            action: "delete repository fixture file",
            source,
        })?;
        Ok(())
    }

    pub(crate) fn replace_fixture_path_with_directory_symlink(
        &mut self,
        link_path: &RepositoryRelativePath,
        target_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        self.replace_fixture_path_with_symlink(link_path, target_path, SymlinkKind::Directory)
    }

    pub(crate) fn replace_fixture_path_with_file_symlink(
        &mut self,
        link_path: &RepositoryRelativePath,
        target_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        self.replace_fixture_path_with_symlink(link_path, target_path, SymlinkKind::File)
    }

    fn replace_fixture_path_with_symlink(
        &mut self,
        link_path: &RepositoryRelativePath,
        target_path: &RepositoryRelativePath,
        kind: SymlinkKind,
    ) -> InstallStepResult<()> {
        let link_absolute = self.repository_path(link_path.as_str())?;
        let target_absolute = self.repository_path(target_path.as_str())?;
        ensure_symlink_target_exists(&target_absolute, kind)?;
        if let Some(parent) = link_absolute.parent() {
            fs::create_dir_all(parent).map_err(|source| InstallStepError::Io {
                path: parent.to_path_buf(),
                action: "create symlink parent directory in repository fixture",
                source,
            })?;
        }
        remove_existing_path_if_present(&link_absolute)?;
        create_symlink(kind, &target_absolute, &link_absolute).map_err(|source| {
            InstallStepError::Io {
                path: link_absolute,
                action: "create repository fixture symlink",
                source,
            }
        })?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum SymlinkKind {
    Directory,
    File,
}

impl Drop for InstallContext {
    fn drop(&mut self) {
        if self.repository_root.as_os_str().is_empty() {
            return;
        }
        if let Err(source) = fs::remove_dir_all(&self.repository_root) {
            let _ = InstallStepError::RemoveFixtureDirectory {
                path: self.repository_root.clone(),
                source,
            };
        }
    }
}

pub(super) fn scenario_repository_root() -> PathBuf {
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = SCENARIO_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "tanren-bdd-install-{}-{sequence}-{now_nanos}",
        std::process::id(),
    ))
}

#[cfg(unix)]
fn create_symlink(kind: SymlinkKind, target: &Path, link: &Path) -> io::Result<()> {
    let _ = kind;
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_symlink(kind: SymlinkKind, target: &Path, link: &Path) -> io::Result<()> {
    match kind {
        SymlinkKind::Directory => std::os::windows::fs::symlink_dir(target, link),
        SymlinkKind::File => std::os::windows::fs::symlink_file(target, link),
    }
}

#[cfg(not(any(unix, windows)))]
compile_error!("install BDD symlink fixture steps require unix or windows support");

fn ensure_symlink_target_exists(target: &Path, kind: SymlinkKind) -> InstallStepResult<()> {
    match kind {
        SymlinkKind::Directory => {
            fs::create_dir_all(target).map_err(|source| InstallStepError::Io {
                path: target.to_path_buf(),
                action: "create symlink target directory in repository fixture",
                source,
            })?;
        }
        SymlinkKind::File => {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|source| InstallStepError::Io {
                    path: parent.to_path_buf(),
                    action: "create symlink target parent directory in repository fixture",
                    source,
                })?;
            }
            if !target.exists() {
                fs::write(target, "").map_err(|source| InstallStepError::Io {
                    path: target.to_path_buf(),
                    action: "create symlink target file in repository fixture",
                    source,
                })?;
            }
        }
    }
    Ok(())
}

fn remove_existing_path_if_present(path: &Path) -> InstallStepResult<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.is_dir() {
                fs::remove_dir_all(path).map_err(|source| InstallStepError::Io {
                    path: path.to_path_buf(),
                    action: "remove existing repository fixture directory before symlink swap",
                    source,
                })?;
            } else {
                fs::remove_file(path).map_err(|source| InstallStepError::Io {
                    path: path.to_path_buf(),
                    action: "remove existing repository fixture file before symlink swap",
                    source,
                })?;
            }
            Ok(())
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(InstallStepError::Io {
            path: path.to_path_buf(),
            action: "inspect repository fixture path before symlink swap",
            source,
        }),
    }
}
