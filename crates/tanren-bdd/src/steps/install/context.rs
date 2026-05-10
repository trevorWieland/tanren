use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use tanren_testkit::{CliCommandOutcome, InstallCommandRequest, InstallHarness};

use crate::steps::install::manifest_helpers::RepositoryRelativePath;
use crate::steps::install::repo_fixture::scenario_repository_root;
use crate::steps::install_snapshot::RepositorySnapshot;

use super::{InstallStepError, InstallStepResult};

/// Per-scenario install fixture state.
///
/// These fields are BDD proof harness inputs and witnesses only. They are not
/// production projection state and never become canonical drift authority.
#[derive(Debug)]
pub(crate) struct InstallContext {
    pub(super) repository_root: PathBuf,
    pub(super) fixture_proof_file_baselines: BTreeMap<RepositoryRelativePath, Vec<u8>>,
    pub(super) fixture_proof_snapshot_before_last_run: Option<RepositorySnapshot>,
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
            fixture_proof_file_baselines: BTreeMap::new(),
            fixture_proof_snapshot_before_last_run: None,
            last_run: None,
        })
    }

    pub(crate) async fn run_install(
        &mut self,
        harness: &mut dyn InstallHarness,
        profile: &str,
        integrations: Option<&str>,
    ) -> InstallStepResult<()> {
        let before = RepositorySnapshot::capture(&self.repository_root)?;
        let outcome = harness
            .run_install(InstallCommandRequest {
                repository_root: self.repository_root.clone(),
                profile: profile.to_owned(),
                integrations: integrations.map(ToOwned::to_owned),
            })
            .await
            .map_err(|source| InstallStepError::RunInstallCommand { source })?;
        self.fixture_proof_snapshot_before_last_run = Some(before);
        self.last_run = Some(outcome);
        Ok(())
    }

    pub(crate) async fn run_drift(
        &mut self,
        harness: &mut dyn InstallHarness,
        profile: &str,
        integrations: Option<&str>,
    ) -> InstallStepResult<()> {
        let before = RepositorySnapshot::capture(&self.repository_root)?;
        let outcome = harness
            .run_drift(InstallCommandRequest {
                repository_root: self.repository_root.clone(),
                profile: profile.to_owned(),
                integrations: integrations.map(ToOwned::to_owned),
            })
            .await
            .map_err(|source| InstallStepError::RunDriftCommand { source })?;
        self.fixture_proof_snapshot_before_last_run = Some(before);
        self.last_run = Some(outcome);
        Ok(())
    }

    pub(super) fn require_last_run(&self) -> InstallStepResult<&InstallCommandOutcome> {
        self.last_run
            .as_ref()
            .ok_or(InstallStepError::InstallCommandNotExecuted)
    }

    pub(super) fn repository_path(&self, relative_path: &RepositoryRelativePath) -> PathBuf {
        self.repository_root.join(relative_path.as_str())
    }
}

pub(super) type InstallCommandOutcome = CliCommandOutcome;
