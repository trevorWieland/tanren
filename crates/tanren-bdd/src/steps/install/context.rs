use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

use tanren_testkit::{
    AccountHarness, CliCommandOutcome, EffectiveConfigurationFixture, InstallProofProfile,
    RUST_CARGO_STANDARDS_ROOT,
};

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
    /// Seeded effective-configuration read-model fixture. Set after install
    /// completes; used by standards-inspect assertions as the expected source
    /// of truth (not the repo projection file).
    pub(super) effective_config_fixture: Option<EffectiveConfigurationFixture>,
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
            effective_config_fixture: None,
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
        if self.last_run.as_ref().is_some_and(|run| run.success) {
            let parsed_profile: InstallProofProfile = profile
                .parse()
                .map_err(|_| InstallStepError::InstallCommandNotExecuted)?;
            self.seed_effective_config_fixture(parsed_profile, RUST_CARGO_STANDARDS_ROOT);
        }
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

    /// Seed the effective-configuration fixture from the install profile and
    /// configured standards root. Called after install succeeds and after
    /// standards assets are relocated.
    pub(super) fn seed_effective_config_fixture(
        &mut self,
        profile: InstallProofProfile,
        standards_root: &str,
    ) {
        self.effective_config_fixture =
            Some(EffectiveConfigurationFixture::new(profile, standards_root));
    }

    /// Borrow the seeded effective-configuration fixture for assertions.
    pub(super) fn effective_config_fixture(
        &self,
    ) -> InstallStepResult<&EffectiveConfigurationFixture> {
        self.effective_config_fixture
            .as_ref()
            .ok_or(InstallStepError::InstallCommandNotExecuted)
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
