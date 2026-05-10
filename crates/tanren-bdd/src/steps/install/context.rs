use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use tanren_testkit::install_contract::{InstallProofIntegration, InstallProofProfile};
use tanren_testkit::{
    CliCommandOutcome, InstallCommandKind, InstallCommandRequest, InstallHarness,
};

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
    pub(super) last_command_kind: Option<InstallCommandKind>,
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
            last_command_kind: None,
        })
    }

    #[tracing::instrument(
        name = "bdd_install_ctx_run_install",
        level = "debug",
        skip(self, harness),
        fields(
            command_kind = %InstallCommandKind::Install,
            harness_kind = tracing::field::Empty,
            profile = %profile.as_str(),
            integration_count = integrations.as_ref().map_or(0, |s| s.len())
        )
    )]
    pub(crate) async fn run_install(
        &mut self,
        harness: &mut dyn InstallHarness,
        profile: InstallProofProfile,
        integrations: Option<&BTreeSet<InstallProofIntegration>>,
        raw_profile: Option<String>,
        raw_integrations: Option<String>,
    ) -> InstallStepResult<()> {
        tracing::Span::current().record("harness_kind", tracing::field::debug(harness.kind()));
        let before = RepositorySnapshot::capture(&self.repository_root)?;
        let outcome = harness
            .run_install(InstallCommandRequest {
                repository_root: self.repository_root.clone(),
                profile,
                integrations: integrations.cloned(),
                raw_profile,
                raw_integrations,
            })
            .await
            .map_err(|source| InstallStepError::RunInstallCommand { source })?;
        self.fixture_proof_snapshot_before_last_run = Some(before);
        self.last_run = Some(outcome);
        self.last_command_kind = Some(InstallCommandKind::Install);
        Ok(())
    }

    #[tracing::instrument(
        name = "bdd_install_ctx_run_drift",
        level = "debug",
        skip(self, harness),
        fields(
            command_kind = %InstallCommandKind::Drift,
            harness_kind = tracing::field::Empty,
            profile = %profile.as_str(),
            integration_count = integrations.as_ref().map_or(0, |s| s.len())
        )
    )]
    pub(crate) async fn run_drift(
        &mut self,
        harness: &mut dyn InstallHarness,
        profile: InstallProofProfile,
        integrations: Option<&BTreeSet<InstallProofIntegration>>,
        raw_profile: Option<String>,
        raw_integrations: Option<String>,
    ) -> InstallStepResult<()> {
        tracing::Span::current().record("harness_kind", tracing::field::debug(harness.kind()));
        let before = RepositorySnapshot::capture(&self.repository_root)?;
        let outcome = harness
            .run_drift(InstallCommandRequest {
                repository_root: self.repository_root.clone(),
                profile,
                integrations: integrations.cloned(),
                raw_profile,
                raw_integrations,
            })
            .await
            .map_err(|source| InstallStepError::RunDriftCommand { source })?;
        self.fixture_proof_snapshot_before_last_run = Some(before);
        self.last_run = Some(outcome);
        self.last_command_kind = Some(InstallCommandKind::Drift);
        Ok(())
    }

    pub(super) fn require_last_run(&self) -> InstallStepResult<&InstallCommandOutcome> {
        self.last_run
            .as_ref()
            .ok_or(InstallStepError::InstallCommandNotExecuted)
    }

    pub(super) fn require_last_command_kind(&self) -> InstallStepResult<InstallCommandKind> {
        self.last_command_kind
            .ok_or(InstallStepError::InstallCommandNotExecuted)
    }

    pub(super) fn repository_path(&self, relative_path: &RepositoryRelativePath) -> PathBuf {
        self.repository_root.join(relative_path.as_str())
    }
}

pub(super) type InstallCommandOutcome = CliCommandOutcome;
