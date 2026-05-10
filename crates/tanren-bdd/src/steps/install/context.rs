use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use tanren_testkit::{AccountHarness, CliCommandOutcome};

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
        harness: &mut dyn AccountHarness,
        profile: &str,
        integrations: Option<&str>,
    ) -> InstallStepResult<()> {
        let before = RepositorySnapshot::capture(&self.repository_root)?;
        let mut args = vec![
            OsString::from("install"),
            OsString::from("--repo"),
            self.repository_root.as_os_str().to_owned(),
            OsString::from("--profile"),
            OsString::from(profile),
        ];
        if let Some(selected) = integrations {
            args.push(OsString::from("--integrations"));
            args.push(OsString::from(selected));
        }
        let outcome = harness
            .execute_cli_command(args)
            .await
            .map_err(|source| InstallStepError::RunInstallCommand { source })?;
        self.snapshot_before_last_run = Some(before);
        self.last_run = Some(outcome);
        Ok(())
    }

    pub(crate) async fn run_standards_inspect(
        &mut self,
        harness: &mut dyn AccountHarness,
    ) -> InstallStepResult<()> {
        let before = RepositorySnapshot::capture(&self.repository_root)?;
        let args = vec![
            OsString::from("standards"),
            OsString::from("inspect"),
            OsString::from("--repo"),
            self.repository_root.as_os_str().to_owned(),
        ];
        let outcome = harness
            .execute_cli_command(args)
            .await
            .map_err(|source| InstallStepError::RunStandardsInspectCommand { source })?;
        self.snapshot_before_last_run = Some(before);
        self.last_run = Some(outcome);
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

pub(super) type InstallCommandOutcome = CliCommandOutcome;
