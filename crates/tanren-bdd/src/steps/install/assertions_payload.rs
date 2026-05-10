use tanren_testkit::{PayloadEntrySummary, load_managed_branch_payload};

use super::context::InstallContext;
use super::{InstallStepError, InstallStepResult};

impl InstallContext {
    pub(crate) fn assert_managed_branch_payload_entries_exist(&self) -> InstallStepResult<()> {
        let payload = load_managed_branch_payload(&self.repository_root)
            .map_err(|source| InstallStepError::InstallProofFailure { source })?;
        for entry in payload.manifest_entries() {
            let exists = payload
                .entry_content_exists(entry)
                .map_err(|source| InstallStepError::InstallProofFailure { source })?;
            if !exists {
                return Err(InstallStepError::ExpectedFileToExist {
                    path: payload.checkout_root().join(entry.path.as_str()),
                });
            }
        }
        Ok(())
    }

    pub(crate) fn assert_payload_profile_is_rust_cargo(&self) -> InstallStepResult<()> {
        let payload = load_managed_branch_payload(&self.repository_root)
            .map_err(|source| InstallStepError::InstallProofFailure { source })?;
        let profile = payload.profile();
        if profile.as_str() != "rust-cargo" {
            return Err(InstallStepError::InstallProofFailure {
                source: tanren_testkit::InstallProofError::ManifestContractViolation {
                    expected: "payload profile must be 'rust-cargo'".to_owned(),
                    manifest_path: std::path::PathBuf::new(),
                    manifest: String::new(),
                },
            });
        }
        Ok(())
    }

    pub(crate) fn assert_payload_contains_methodology_command_entries(
        &self,
    ) -> InstallStepResult<()> {
        let payload = load_managed_branch_payload(&self.repository_root)
            .map_err(|source| InstallStepError::InstallProofFailure { source })?;
        let has_commands = payload.manifest_entries().iter().any(|entry| {
            let summary = PayloadEntrySummary::from_manifest_entry(entry);
            summary.asset_class() == "MethodologyCommand"
        });
        if !has_commands {
            return Err(InstallStepError::InstallProofFailure {
                source: tanren_testkit::InstallProofError::ManifestContractViolation {
                    expected: "payload must contain methodology-command entries".to_owned(),
                    manifest_path: std::path::PathBuf::new(),
                    manifest: String::new(),
                },
            });
        }
        Ok(())
    }

    pub(crate) fn assert_payload_contains_standards_entries(&self) -> InstallStepResult<()> {
        let payload = load_managed_branch_payload(&self.repository_root)
            .map_err(|source| InstallStepError::InstallProofFailure { source })?;
        let has_standards = payload.manifest_entries().iter().any(|entry| {
            let summary = PayloadEntrySummary::from_manifest_entry(entry);
            summary.asset_class() == "StandardsProfile"
        });
        if !has_standards {
            return Err(InstallStepError::InstallProofFailure {
                source: tanren_testkit::InstallProofError::ManifestContractViolation {
                    expected: "payload must contain standards-profile entries".to_owned(),
                    manifest_path: std::path::PathBuf::new(),
                    manifest: String::new(),
                },
            });
        }
        Ok(())
    }
}
