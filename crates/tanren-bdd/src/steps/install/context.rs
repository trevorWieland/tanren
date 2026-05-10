use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use tanren_testkit::{AccountHarness, CliCommandOutcome, InstallProofRepositorySnapshot};

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
    pub(super) uninstall_snapshot_before_last_run: Option<InstallProofRepositorySnapshot>,
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
            uninstall_snapshot_before_last_run: None,
            last_run: None,
        })
    }

    pub(crate) async fn run_install(
        &mut self,
        harness: &mut dyn AccountHarness,
        profile: &str,
        integrations: Option<&str>,
    ) -> InstallStepResult<()> {
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
        self.run_cli_command(harness, args, InstallCommandKind::Install)
            .await
    }

    pub(crate) async fn run_uninstall_preview(
        &mut self,
        harness: &mut dyn AccountHarness,
    ) -> InstallStepResult<()> {
        let args = vec![
            OsString::from("uninstall"),
            OsString::from("--repo"),
            self.repository_root.as_os_str().to_owned(),
        ];
        self.run_cli_command(harness, args, InstallCommandKind::UninstallPreview)
            .await
    }

    pub(crate) async fn run_uninstall_apply(
        &mut self,
        harness: &mut dyn AccountHarness,
    ) -> InstallStepResult<()> {
        let args = vec![
            OsString::from("uninstall"),
            OsString::from("--repo"),
            self.repository_root.as_os_str().to_owned(),
            OsString::from("--confirm"),
        ];
        self.run_cli_command(harness, args, InstallCommandKind::UninstallApply)
            .await
    }

    async fn run_cli_command(
        &mut self,
        harness: &mut dyn AccountHarness,
        args: Vec<OsString>,
        command_kind: InstallCommandKind,
    ) -> InstallStepResult<()> {
        let before = RepositorySnapshot::capture(&self.repository_root)?;
        let uninstall_before =
            super::manifest_helpers::capture_uninstall_repository_snapshot(&self.repository_root)?;
        let harness_name = harness.kind().as_str().to_owned();
        let rendered_args = render_cli_args(&args);
        let outcome =
            harness
                .execute_cli_command(args)
                .await
                .map_err(|source| match command_kind {
                    InstallCommandKind::Install => InstallStepError::RunInstallCommand {
                        harness: harness_name.clone(),
                        args: rendered_args.clone(),
                        source,
                    },
                    InstallCommandKind::UninstallPreview => {
                        InstallStepError::RunUninstallPreviewCommand {
                            harness: harness_name.clone(),
                            args: rendered_args.clone(),
                            source,
                        }
                    }
                    InstallCommandKind::UninstallApply => {
                        InstallStepError::RunUninstallApplyCommand {
                            harness: harness_name.clone(),
                            args: rendered_args.clone(),
                            source,
                        }
                    }
                })?;
        self.snapshot_before_last_run = Some(before);
        self.uninstall_snapshot_before_last_run = Some(uninstall_before);
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

#[derive(Debug, Clone, Copy)]
enum InstallCommandKind {
    Install,
    UninstallPreview,
    UninstallApply,
}

fn render_cli_args(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}
