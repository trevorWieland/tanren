//! Previous-manifest entry contract validation for install/upgrade planning.

use crate::install::catalog::{
    is_generated_integration_destination, standards_profile_destination_root,
};
use crate::install::error::InstallError;
use crate::install::manifest::{AssetClass, ManifestEntry, PreservationPolicy};
use crate::install::{InstallIntegration, InstallProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ManifestEntryKind {
    GeneratedMethodologyCommand { integration: InstallIntegration },
    PreservedStandardsProfile,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ValidatedManifestEntry<'a> {
    pub(super) entry: &'a ManifestEntry,
    pub(super) kind: ManifestEntryKind,
}

pub(super) fn validate_manifest_entry_contract(
    entry: &ManifestEntry,
    profile: InstallProfile,
    manifest_path: &str,
) -> Result<ManifestEntryKind, InstallError> {
    match entry.asset_class {
        AssetClass::MethodologyCommand => {
            let Some(integration) = entry.integration else {
                return Err(InstallError::InvalidInstallManifest {
                    path: manifest_path.to_owned(),
                    message: format!(
                        "methodology-command entry '{}' must include integration",
                        entry.path.as_str()
                    ),
                });
            };
            if entry.preservation != PreservationPolicy::ReplaceGenerated {
                return Err(InstallError::InvalidInstallManifest {
                    path: manifest_path.to_owned(),
                    message: format!(
                        "methodology-command entry '{}' must use replace-generated preservation",
                        entry.path.as_str()
                    ),
                });
            }
            if !is_generated_integration_destination(&entry.path, integration) {
                return Err(InstallError::InvalidInstallManifest {
                    path: manifest_path.to_owned(),
                    message: format!(
                        "methodology-command entry '{}' does not match integration destination for '{}'",
                        entry.path.as_str(),
                        integration.as_str()
                    ),
                });
            }
            Ok(ManifestEntryKind::GeneratedMethodologyCommand { integration })
        }
        AssetClass::StandardsProfile => {
            if entry.integration.is_some() {
                return Err(InstallError::InvalidInstallManifest {
                    path: manifest_path.to_owned(),
                    message: format!(
                        "standards-profile entry '{}' cannot include integration",
                        entry.path.as_str()
                    ),
                });
            }
            if entry.preservation != PreservationPolicy::PreserveUserEdits {
                return Err(InstallError::InvalidInstallManifest {
                    path: manifest_path.to_owned(),
                    message: format!(
                        "standards-profile entry '{}' must use preserve-user-edits preservation",
                        entry.path.as_str()
                    ),
                });
            }
            let expected_root = standards_profile_destination_root(profile);
            if !entry.path.as_str().starts_with(expected_root) {
                return Err(InstallError::InvalidInstallManifest {
                    path: manifest_path.to_owned(),
                    message: format!(
                        "standards-profile entry '{}' must be under '{expected_root}'",
                        entry.path.as_str()
                    ),
                });
            }
            Ok(ManifestEntryKind::PreservedStandardsProfile)
        }
    }
}
