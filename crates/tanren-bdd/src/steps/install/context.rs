use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use tanren_testkit::locate_workspace_binary;
use tokio::process::Command;

use crate::steps::install::manifest_helpers::RepositoryRelativePath;
use crate::steps::install::repo_fixture::scenario_repository_root;
use crate::steps::install_snapshot::RepositorySnapshot;

use super::{InstallStepError, InstallStepResult};

/// Per-scenario install fixture state.
#[derive(Debug)]
pub(crate) struct InstallContext {
    pub(super) repository_root: PathBuf,
    pub(super) baselines: BTreeMap<RepositoryRelativePath, Vec<u8>>,
    pub(super) snapshot_before_last_run: Option<RepositorySnapshot>,
    pub(super) last_run: Option<InstallCommandOutcome>,
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

    pub(super) fn require_last_run(&self) -> InstallStepResult<&InstallCommandOutcome> {
        self.last_run
            .as_ref()
            .ok_or(InstallStepError::InstallCommandNotExecuted)
    }

    pub(super) fn repository_path(&self, relative_path: &str) -> InstallStepResult<PathBuf> {
        super::manifest_helpers::validate_relative_path(relative_path)?;
        Ok(self.repository_root.join(relative_path))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InstallCommandOutcome {
    pub(super) status_code: Option<i32>,
    pub(super) success: bool,
    pub(super) stdout: String,
    pub(super) stderr: String,
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
