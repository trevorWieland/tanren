//! Versioned manifest migration layer for schema evolution.
//!
//! Each supported migration step is a named enum variant so the upgrade
//! preview can report exactly which transformations were applied.
//! Unsupported future versions produce a typed compatibility concern
//! before any destructive action proceeds.

use crate::install::error::InstallError;
use crate::install::manifest::{INSTALL_MANIFEST_VERSION, InstallManifest, ManifestVersion};
use crate::install::manifest_entry_contract::validate_manifest_entry_contract;

/// Minimum manifest version that can be migrated to the current schema.
pub(super) const MIN_SUPPORTED_MANIFEST_VERSION: ManifestVersion = ManifestVersion::new(1);

/// Named migration steps for install manifest schema evolution.
///
/// Each variant corresponds to a single forward migration step. Steps are
/// applied in order from the detected version up to the current version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum ManifestMigrationStep {}

/// Outcome of a manifest migration attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ManifestMigrationOutcome {
    /// Manifest is already at the current schema version.
    CurrentVersion { manifest: InstallManifest },
    /// Manifest was migrated from a supported prior version.
    Migrated {
        steps: Vec<ManifestMigrationStep>,
        manifest: InstallManifest,
    },
    /// Manifest version is not supported for migration.
    UnsupportedVersion {
        detected: ManifestVersion,
        min_supported: ManifestVersion,
        current: ManifestVersion,
    },
}

/// Attempt to migrate an install manifest to the current schema version.
///
/// Returns [`ManifestMigrationOutcome::CurrentVersion`] when the manifest is
/// already at the current version, [`ManifestMigrationOutcome::Migrated`]
/// when named migration steps were applied, or
/// [`ManifestMigrationOutcome::UnsupportedVersion`] when the detected version
/// is outside the supported migration range.
pub(super) fn migrate_manifest(manifest: InstallManifest) -> ManifestMigrationOutcome {
    let detected = manifest.manifest_version;

    if detected == INSTALL_MANIFEST_VERSION {
        return ManifestMigrationOutcome::CurrentVersion { manifest };
    }

    if detected < MIN_SUPPORTED_MANIFEST_VERSION || detected > INSTALL_MANIFEST_VERSION {
        return ManifestMigrationOutcome::UnsupportedVersion {
            detected,
            min_supported: MIN_SUPPORTED_MANIFEST_VERSION,
            current: INSTALL_MANIFEST_VERSION,
        };
    }

    // Apply migration steps from detected version up to current.
    // Currently only v1 exists, so this branch is unreachable in
    // production. Future versions will add named steps here.
    let steps = Vec::new();
    let mut migrated = manifest;
    migrated.manifest_version = INSTALL_MANIFEST_VERSION;

    ManifestMigrationOutcome::Migrated {
        steps,
        manifest: migrated,
    }
}

/// Validate a migrated or current-version manifest for structural invariants.
///
/// Checks integrations non-emptiness, duplicate entry paths, and per-entry
/// contract constraints (entry kind, preservation policy, hash format).
pub(super) fn validate_migrated_manifest(
    manifest: InstallManifest,
    manifest_path: &str,
) -> Result<ValidatedInstallManifest, InstallError> {
    if manifest.integrations.is_empty() {
        return Err(InstallError::InvalidInstallManifest {
            path: manifest_path.to_owned(),
            message: "manifest integrations list cannot be empty".to_owned(),
        });
    }

    let mut seen_paths = std::collections::BTreeSet::new();
    for entry in &manifest.entries {
        if !seen_paths.insert(entry.path.as_str()) {
            return Err(InstallError::InvalidInstallManifest {
                path: manifest_path.to_owned(),
                message: format!("duplicate entry path '{}'", entry.path.as_str()),
            });
        }
    }

    for entry in &manifest.entries {
        validate_manifest_entry_contract(entry, manifest.profile, manifest_path)?;
    }

    Ok(ValidatedInstallManifest { manifest })
}

/// A manifest that has passed all structural and contract validations.
///
/// Constructed exclusively through [`validate_migrated_manifest`], which
/// checks manifest version, profile, integrations, duplicate paths, entry
/// kind, preservation policy, and hash formats. Callers can rely on the
/// invariants enforced at construction time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ValidatedInstallManifest {
    manifest: InstallManifest,
}

impl ValidatedInstallManifest {
    /// Borrow the underlying install manifest.
    #[must_use]
    pub(super) fn inner(&self) -> &InstallManifest {
        &self.manifest
    }
}

/// Outcome of loading and migrating a repository install manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ManifestLoadOutcome {
    /// No previous manifest was found in the repository.
    NoManifest,
    /// Manifest was migrated (if needed) and fully validated.
    Validated(ValidatedInstallManifest),
    /// Manifest version is not supported for migration.
    UnsupportedVersion {
        detected: ManifestVersion,
        min_supported: ManifestVersion,
        current: ManifestVersion,
    },
}

impl ManifestLoadOutcome {
    /// Require a validated manifest or error (used by the install path).
    ///
    /// Converts `UnsupportedVersion` to [`InstallError::InvalidInstallManifest`]
    /// and `NoManifest` to `None`.
    pub(super) fn require_validated(
        self,
        manifest_path: &str,
    ) -> Result<Option<ValidatedInstallManifest>, InstallError> {
        match self {
            Self::NoManifest => Ok(None),
            Self::Validated(v) => Ok(Some(v)),
            Self::UnsupportedVersion { detected, .. } => {
                Err(InstallError::InvalidInstallManifest {
                    path: manifest_path.to_owned(),
                    message: format!("unsupported manifest version {detected}"),
                })
            }
        }
    }
}
