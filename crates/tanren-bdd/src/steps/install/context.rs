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
    pub(super) labeled_snapshots: BTreeMap<String, RepositorySnapshot>,
    pub(super) snapshot_before_last_run: Option<RepositorySnapshot>,
    pub(super) last_run: Option<InstallCommandOutcome>,
    pub(super) last_preview_id: Option<String>,
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
            labeled_snapshots: BTreeMap::new(),
            snapshot_before_last_run: None,
            last_run: None,
            last_preview_id: None,
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

    pub(crate) async fn run_upgrade(
        &mut self,
        harness: &mut dyn AccountHarness,
        confirm: bool,
    ) -> InstallStepResult<()> {
        let before = RepositorySnapshot::capture(&self.repository_root)?;
        let mut args = vec![
            OsString::from("upgrade"),
            OsString::from("--repo"),
            self.repository_root.as_os_str().to_owned(),
        ];
        if confirm {
            args.push(OsString::from("--confirm"));
        }
        let outcome = harness
            .execute_cli_command(args)
            .await
            .map_err(|source| InstallStepError::RunUpgradeCommand { source })?;
        self.snapshot_before_last_run = Some(before);
        self.last_run = Some(outcome);
        Ok(())
    }

    pub(crate) fn capture_labeled_snapshot(&mut self, label: &str) -> InstallStepResult<()> {
        let label = label.trim().to_owned();
        if label.is_empty() {
            return Err(InstallStepError::EmptySnapshotLabel);
        }
        let snapshot = RepositorySnapshot::capture(&self.repository_root)?;
        self.labeled_snapshots.insert(label, snapshot);
        Ok(())
    }

    pub(crate) fn seed_upgrade_fixture_from_install(
        &mut self,
        snapshot_label: &str,
    ) -> InstallStepResult<()> {
        self.assert_success()?;
        self.capture_labeled_snapshot(snapshot_label)
    }

    pub(crate) fn capture_preview_id_from_stdout(&mut self) {
        if let Some(ref run) = self.last_run {
            self.last_preview_id = extract_preview_id_value(&run.stdout);
        }
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

fn extract_preview_id_value(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("status=preview command=upgrade") {
            for field in rest.split(' ') {
                if let Some(value) = field.strip_prefix("preview_id=") {
                    return Some(value.to_owned());
                }
            }
        }
        if let Some(rest) = line.strip_prefix("status=confirmation_required command=upgrade") {
            for field in rest.split(' ') {
                if let Some(value) = field.strip_prefix("preview_id=") {
                    return Some(value.to_owned());
                }
            }
        }
    }
    None
}
