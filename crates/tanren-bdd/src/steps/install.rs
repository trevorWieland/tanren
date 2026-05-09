//! Install-flow step definitions for B-0068 / B-0070.
//!
//! The steps execute the real `tanren-cli` binary against a per-scenario
//! temporary repository fixture. No installer internals are called directly.

pub(crate) use crate::steps::install_error::{InstallStepError, InstallStepResult};
use crate::steps::install_helpers;
use crate::steps::install_helpers::RepositoryRelativePath;
use crate::steps::install_snapshot::RepositorySnapshot;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tanren_testkit::locate_workspace_binary;
use tokio::process::Command;

static SCENARIO_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Per-scenario install fixture state.
#[derive(Debug)]
pub(crate) struct InstallContext {
    repository_root: PathBuf,
    baselines: BTreeMap<RepositoryRelativePath, Vec<u8>>,
    snapshot_before_last_run: Option<RepositorySnapshot>,
    last_run: Option<InstallCommandOutcome>,
}

impl InstallContext {
    pub(crate) fn new() -> InstallStepResult<Self> {
        let repository_root = scenario_repository_root();
        fs::create_dir_all(&repository_root).map_err(|source| {
            InstallStepError::CreateFixtureDirectory {
                path: repository_root.clone(),
                source,
            }
        })?;
        Ok(Self {
            repository_root,
            baselines: BTreeMap::new(),
            snapshot_before_last_run: None,
            last_run: None,
        })
    }

    pub(crate) async fn run_install(
        &mut self,
        profile: &str,
        integrations: Option<&str>,
    ) -> InstallStepResult<()> {
        let before = RepositorySnapshot::capture(&self.repository_root)?;
        let binary = locate_workspace_binary("tanren-cli")
            .map_err(|source| InstallStepError::LocateCliBinary { source })?;

        let mut command = Command::new(binary);
        command
            .arg("install")
            .arg("--repo")
            .arg(&self.repository_root)
            .arg("--profile")
            .arg(profile);
        if let Some(selected) = integrations {
            command.arg("--integrations").arg(selected);
        }

        let output = command
            .output()
            .await
            .map_err(|source| InstallStepError::RunInstallSubprocess { source })?;
        self.snapshot_before_last_run = Some(before);
        self.last_run = Some(InstallCommandOutcome::from(output));
        Ok(())
    }

    pub(crate) fn assert_success(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.success {
            return Err(InstallStepError::InstallCommandExpectedSuccess {
                status: run.status_code,
                stdout: run.stdout.clone(),
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_nonzero(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if run.success {
            return Err(InstallStepError::InstallCommandExpectedFailure {
                stdout: run.stdout.clone(),
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_summary_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        ensure_stdout_contains(run, "status=ok command=install")?;
        for field in [
            "created=",
            "updated=",
            "removed=",
            "restored=",
            "preserved=",
        ] {
            ensure_stdout_contains(run, field)?;
        }
        for section in [
            "paths created=[",
            "updated=[",
            "removed=[",
            "restored=[",
            "preserved=[",
        ] {
            ensure_stdout_contains(run, section)?;
        }
        Ok(())
    }

    pub(crate) fn assert_validation_failure_output(&self) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.stderr.contains("error: validation_failed") {
            return Err(InstallStepError::ValidationFailureMissing {
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn assert_no_writes_since_last_run(&self) -> InstallStepResult<()> {
        let before = self
            .snapshot_before_last_run
            .as_ref()
            .ok_or(InstallStepError::MissingSnapshotBeforeRun)?;
        let after = RepositorySnapshot::capture(&self.repository_root)?;
        if &after != before {
            return Err(InstallStepError::RepositorySnapshotMismatch);
        }
        Ok(())
    }

    pub(crate) fn assert_stderr_contains(&self, expected: &str) -> InstallStepResult<()> {
        let run = self.require_last_run()?;
        if !run.stderr.contains(expected) {
            return Err(InstallStepError::StderrMissingExpected {
                expected: expected.to_owned(),
                stderr: run.stderr.clone(),
            });
        }
        Ok(())
    }

    pub(crate) fn write_fixture_file(
        &mut self,
        relative_path: &RepositoryRelativePath,
        content: String,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                install_helpers::io_error(
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

    pub(crate) fn assert_file_exists(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        if !absolute.exists() {
            return Err(InstallStepError::ExpectedFileToExist { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_file_absent(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        if absolute.exists() {
            return Err(InstallStepError::ExpectedFileToBeAbsent { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_exact_file_content(
        &self,
        relative_path: &RepositoryRelativePath,
        expected: String,
    ) -> InstallStepResult<()> {
        let absolute = self.repository_path(relative_path.as_str())?;
        let bytes = fs::read(&absolute).map_err(|source| InstallStepError::ReadFile {
            path: absolute.clone(),
            action: "read repository fixture file",
            source,
        })?;
        if bytes != expected.into_bytes() {
            return Err(InstallStepError::UnexpectedFileContent { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_file_content_preserved(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let baseline =
            self.baselines
                .get(relative_path)
                .ok_or_else(|| InstallStepError::MissingBaseline {
                    path: relative_path.as_str().to_owned(),
                })?;
        let absolute = self.repository_path(relative_path.as_str())?;
        let bytes = fs::read(&absolute).map_err(|source| InstallStepError::ReadFile {
            path: absolute.clone(),
            action: "read repository fixture file",
            source,
        })?;
        if &bytes != baseline {
            return Err(InstallStepError::FileContentChanged { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_file_content_replaced(
        &self,
        relative_path: &RepositoryRelativePath,
    ) -> InstallStepResult<()> {
        let baseline =
            self.baselines
                .get(relative_path)
                .ok_or_else(|| InstallStepError::MissingBaseline {
                    path: relative_path.as_str().to_owned(),
                })?;
        let absolute = self.repository_path(relative_path.as_str())?;
        let bytes = fs::read(&absolute).map_err(|source| InstallStepError::ReadFile {
            path: absolute.clone(),
            action: "read repository fixture file",
            source,
        })?;
        if &bytes == baseline {
            return Err(InstallStepError::FileContentNotReplaced { path: absolute });
        }
        Ok(())
    }

    pub(crate) fn assert_rust_cargo_default_assets_installed(&self) -> InstallStepResult<()> {
        install_helpers::assert_rust_cargo_default_assets_installed(&self.repository_root)
    }

    pub(crate) fn assert_rust_cargo_standards_installed(&self) -> InstallStepResult<()> {
        install_helpers::assert_rust_cargo_standards_installed(&self.repository_root)
    }

    pub(crate) fn assert_selected_integration_command_assets(
        &self,
        integrations: &str,
    ) -> InstallStepResult<()> {
        let selected = integrations
            .split(',')
            .map(str::trim)
            .filter(|integration| !integration.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        install_helpers::assert_selected_integration_command_assets(
            &self.repository_root,
            &selected,
        )
    }

    pub(crate) fn assert_manifest_rust_cargo_defaults(&self) -> InstallStepResult<()> {
        install_helpers::assert_manifest_rust_cargo_defaults(&self.repository_root)
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
        install_helpers::append_stale_generated_manifest_entry(
            &mut manifest,
            relative_path,
            install_helpers::sha256_hex(&stale_bytes),
        );
        fs::write(&manifest_path, manifest).map_err(|source| InstallStepError::WriteFile {
            path: manifest_path,
            action: "write install manifest with stale entry",
            source,
        })?;
        Ok(())
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
        let link_absolute = self.repository_path(link_path.as_str())?;
        let target_absolute = self.repository_path(target_path.as_str())?;
        fs::create_dir_all(&target_absolute).map_err(|source| InstallStepError::Io {
            path: target_absolute.clone(),
            action: "create symlink target directory in repository fixture",
            source,
        })?;
        if let Some(parent) = link_absolute.parent() {
            fs::create_dir_all(parent).map_err(|source| InstallStepError::Io {
                path: parent.to_path_buf(),
                action: "create symlink parent directory in repository fixture",
                source,
            })?;
        }
        match fs::symlink_metadata(&link_absolute) {
            Ok(metadata) => {
                if metadata.is_dir() {
                    fs::remove_dir_all(&link_absolute).map_err(|source| InstallStepError::Io {
                        path: link_absolute.clone(),
                        action: "remove existing repository fixture directory before symlink swap",
                        source,
                    })?;
                } else {
                    fs::remove_file(&link_absolute).map_err(|source| InstallStepError::Io {
                        path: link_absolute.clone(),
                        action: "remove existing repository fixture file before symlink swap",
                        source,
                    })?;
                }
            }
            Err(source) if source.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(InstallStepError::Io {
                    path: link_absolute,
                    action: "inspect repository fixture path before symlink swap",
                    source,
                });
            }
        }
        create_directory_symlink(&target_absolute, &link_absolute).map_err(|source| {
            InstallStepError::Io {
                path: link_absolute,
                action: "create repository fixture symlink",
                source,
            }
        })?;
        Ok(())
    }

    fn repository_path(&self, relative_path: &str) -> InstallStepResult<PathBuf> {
        install_helpers::validate_relative_path(relative_path)?;
        Ok(self.repository_root.join(relative_path))
    }

    fn require_last_run(&self) -> InstallStepResult<&InstallCommandOutcome> {
        self.last_run
            .as_ref()
            .ok_or(InstallStepError::InstallCommandNotExecuted)
    }
}

impl Default for InstallContext {
    fn default() -> Self {
        Self {
            repository_root: PathBuf::new(),
            baselines: BTreeMap::new(),
            snapshot_before_last_run: None,
            last_run: None,
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstallCommandOutcome {
    status_code: Option<i32>,
    success: bool,
    stdout: String,
    stderr: String,
}

impl From<std::process::Output> for InstallCommandOutcome {
    fn from(output: std::process::Output) -> Self {
        Self {
            status_code: output.status.code(),
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

fn scenario_repository_root() -> PathBuf {
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = SCENARIO_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "tanren-bdd-install-{}-{sequence}-{now_nanos}",
        std::process::id(),
    ))
}

fn ensure_stdout_contains(run: &InstallCommandOutcome, expected: &str) -> InstallStepResult<()> {
    if !run.stdout.contains(expected) {
        return Err(InstallStepError::StdoutMissingExpected {
            expected: expected.to_owned(),
            stdout: run.stdout.clone(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn create_directory_symlink(target: &std::path::Path, link: &std::path::Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_symlink(target: &std::path::Path, link: &std::path::Path) -> io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(not(any(unix, windows)))]
compile_error!("install BDD symlink fixture steps require unix or windows support");
