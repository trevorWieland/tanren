//! Managed branch payload model for delivery-owned install proof assertions.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::install::InstallIntegration;
use crate::install::InstallProfile;
use crate::install::manifest::{AssetClass, ManifestEntry, PreservationPolicy};

use super::InstallProofError;
use super::ObservedInstallManifest;
use super::contract_fs;

/// Delivery-owned managed branch payload model for install proof assertions.
///
/// Represents the typed delivery surface that assertions validate against:
/// manifest entries and their associated payload contents from a source-control
/// managed branch checkout. This is the modeled delivery contract — not raw
/// `repository_root` existence/absence.
///
/// Per architecture invariant 7, repository onboarding is source-control based:
/// Tanren bootstraps a repo by creating and managing a source-control branch/PR.
/// Per invariant 8, repo-local assets are projections or controlled install
/// artifacts generated from typed state. The `ManagedBranchPayload` models the
/// checked-out branch contents after the source-control delivery step, providing
/// a typed surface for BDD assertions instead of direct local filesystem reads.
#[derive(Debug, Clone)]
pub struct ManagedBranchPayload {
    checkout_root: PathBuf,
    pub(super) manifest: ObservedInstallManifest,
}

impl ManagedBranchPayload {
    /// Materialize a managed branch payload from a source-control checkout root.
    ///
    /// Reads and parses the install manifest from the checkout, producing the
    /// typed delivery surface for assertions. The checkout root represents the
    /// modeled branch contents after the source-control delivery step.
    pub fn from_checkout_root(checkout_root: &Path) -> Result<Self, InstallProofError> {
        let manifest = super::read_install_manifest(checkout_root)?;
        Ok(Self {
            checkout_root: checkout_root.to_path_buf(),
            manifest,
        })
    }

    /// The checkout root for the managed branch contents.
    #[must_use]
    pub fn checkout_root(&self) -> &Path {
        &self.checkout_root
    }

    /// The parsed install manifest entries from the managed branch.
    #[must_use]
    pub fn manifest_entries(&self) -> &[ManifestEntry] {
        &self.manifest.parsed.entries
    }

    /// The install profile recorded in the manifest.
    #[must_use]
    pub fn profile(&self) -> InstallProfile {
        self.manifest.parsed.profile
    }

    /// The integrations recorded in the manifest.
    #[must_use]
    pub fn integrations(&self) -> &[InstallIntegration] {
        &self.manifest.parsed.integrations
    }

    /// Filtered manifest entries by asset class.
    #[must_use]
    pub fn entries_by_class(&self, class: AssetClass) -> Vec<&ManifestEntry> {
        self.manifest
            .parsed
            .entries
            .iter()
            .filter(|entry| entry.asset_class == class)
            .collect()
    }

    /// Filtered manifest entries by integration.
    #[must_use]
    pub fn entries_by_integration(&self, integration: InstallIntegration) -> Vec<&ManifestEntry> {
        self.manifest
            .parsed
            .entries
            .iter()
            .filter(|entry| entry.integration == Some(integration))
            .collect()
    }

    /// Entries with the `ReplaceGenerated` preservation policy.
    #[must_use]
    pub fn replace_generated_entries(&self) -> Vec<&ManifestEntry> {
        self.manifest
            .parsed
            .entries
            .iter()
            .filter(|entry| entry.preservation == PreservationPolicy::ReplaceGenerated)
            .collect()
    }

    /// Check whether a payload entry's content exists in the branch checkout.
    pub fn entry_content_exists(&self, entry: &ManifestEntry) -> Result<bool, InstallProofError> {
        let absolute = self.checkout_root.join(entry.path.as_str());
        Ok(absolute.exists())
    }

    /// Read the content of a payload entry from the branch checkout.
    pub fn read_entry_content(&self, entry: &ManifestEntry) -> Result<String, InstallProofError> {
        let absolute = self.checkout_root.join(entry.path.as_str());
        contract_fs::read_to_string_with_context(
            &absolute,
            "read managed branch payload entry content",
        )
    }

    /// Assert a payload entry's content exists in the branch checkout.
    pub fn assert_entry_exists(&self, entry: &ManifestEntry) -> Result<(), InstallProofError> {
        if !self.entry_content_exists(entry)? {
            return Err(InstallProofError::ExpectedPayloadEntryToExist {
                path: self.checkout_root.join(entry.path.as_str()),
            });
        }
        Ok(())
    }
}

/// Delivery-owned manifest entry summary for branch payload wire representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayloadEntrySummary {
    path: String,
    asset_class: String,
    integration: Option<String>,
    preservation: String,
}

impl PayloadEntrySummary {
    /// Build a summary from a manifest entry.
    #[must_use]
    pub fn from_manifest_entry(entry: &ManifestEntry) -> Self {
        Self {
            path: entry.path.as_str().to_owned(),
            asset_class: format!("{:?}", entry.asset_class),
            integration: entry
                .integration
                .map(|integration| integration.as_str().to_owned()),
            preservation: format!("{:?}", entry.preservation),
        }
    }

    /// The repo-relative path of this payload entry.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The asset class of this payload entry.
    #[must_use]
    pub fn asset_class(&self) -> &str {
        &self.asset_class
    }

    /// The integration target, if any.
    #[must_use]
    pub fn integration(&self) -> Option<&str> {
        self.integration.as_deref()
    }

    /// The preservation policy of this payload entry.
    #[must_use]
    pub fn preservation(&self) -> &str {
        &self.preservation
    }
}
