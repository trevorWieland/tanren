use serde::Serialize;

use super::InstallProofError;
use crate::install::InstallIntegration;
use crate::install::manifest::{AssetClass, PreservationPolicy, RepoRelativePath};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LegacyGeneratedMigrationAdapter {
    path: String,
    content_hash: String,
    integration: InstallIntegration,
}

impl LegacyGeneratedMigrationAdapter {
    #[must_use]
    pub(super) fn from_repo_relative_path(path: &RepoRelativePath, content_hash: &str) -> Self {
        Self {
            path: path.as_str().to_owned(),
            content_hash: content_hash.to_owned(),
            integration: InstallIntegration::Codex,
        }
    }

    #[must_use]
    pub(super) fn from_raw_path(path: &str, content_hash: &str) -> Self {
        Self {
            path: path.to_owned(),
            content_hash: content_hash.to_owned(),
            integration: InstallIntegration::Codex,
        }
    }

    pub(super) fn append_to_manifest(
        &self,
        manifest: &mut String,
    ) -> Result<(), InstallProofError> {
        let entry = LegacyGeneratedManifestEntryToml {
            path: self.path.as_str(),
            content_hash: self.content_hash.as_str(),
            asset_class: AssetClass::MethodologyCommand,
            integration: Some(self.integration),
            preservation: PreservationPolicy::ReplaceGenerated,
        };
        let append = LegacyGeneratedManifestEntriesToml {
            entries: vec![entry],
        };
        let toml_fragment = toml::to_string(&append)
            .map_err(|source| InstallProofError::InstallManifestTomlSerialize { source })?;
        manifest.push('\n');
        manifest.push_str(&toml_fragment);
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct LegacyGeneratedManifestEntriesToml<'a> {
    entries: Vec<LegacyGeneratedManifestEntryToml<'a>>,
}

#[derive(Debug, Serialize)]
struct LegacyGeneratedManifestEntryToml<'a> {
    path: &'a str,
    content_hash: &'a str,
    asset_class: AssetClass,
    integration: Option<InstallIntegration>,
    preservation: PreservationPolicy,
}
