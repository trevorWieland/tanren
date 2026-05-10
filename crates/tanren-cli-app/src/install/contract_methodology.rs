use std::path::Path;

use tanren_configuration_secrets::{
    MethodologyProfile, PROJECT_METHODOLOGY_SCHEMA_VERSION, ProjectMethodologyConfig,
};

use crate::install::manifest::{AssetClass, PROJECT_METHODOLOGY_CONFIG_REPO_PATH};

use super::{
    InstallProofError, ObservedInstallManifest, RUST_CARGO_STANDARDS_ROOT, contract_fs,
    manifest_contract_error,
};

pub(super) fn assert_project_methodology_config_for_rust_cargo(
    repository_root: &Path,
    manifest: &ObservedInstallManifest,
) -> Result<(), InstallProofError> {
    let mut config_entries = manifest
        .parsed
        .entries
        .iter()
        .filter(|entry| entry.asset_class == AssetClass::MethodologyConfig);
    let Some(entry) = config_entries.next() else {
        return Err(manifest_contract_error(
            manifest,
            "manifest must include one methodology-config entry",
        ));
    };
    if config_entries.next().is_some() {
        return Err(manifest_contract_error(
            manifest,
            "manifest must include exactly one methodology-config entry",
        ));
    }
    if entry.path.as_str() != PROJECT_METHODOLOGY_CONFIG_REPO_PATH {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "methodology-config entry must target '{}', got '{}'",
                PROJECT_METHODOLOGY_CONFIG_REPO_PATH,
                entry.path.as_str()
            ),
        ));
    }
    if entry.integration.is_some() {
        return Err(manifest_contract_error(
            manifest,
            "methodology-config entry must not declare an integration",
        ));
    }
    contract_fs::assert_file_exists(repository_root, entry.path.as_str())?;

    let absolute_path = repository_root.join(entry.path.as_str());
    let raw = contract_fs::read_to_string_with_context(
        &absolute_path,
        "read project methodology config from fixture",
    )?;
    let config = ProjectMethodologyConfig::from_toml(&raw).map_err(|source| {
        InstallProofError::ProjectMethodologyConfigTomlParse {
            path: absolute_path.clone(),
            source,
        }
    })?;
    if config.schema_version != PROJECT_METHODOLOGY_SCHEMA_VERSION {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "project methodology config schema_version must equal {}, got {}",
                PROJECT_METHODOLOGY_SCHEMA_VERSION, config.schema_version
            ),
        ));
    }
    if config.profile != MethodologyProfile::RustCargo {
        return Err(manifest_contract_error(
            manifest,
            "project methodology config profile must be 'rust-cargo'",
        ));
    }
    if config.standards_root.as_str() != RUST_CARGO_STANDARDS_ROOT {
        return Err(manifest_contract_error(
            manifest,
            &format!(
                "project methodology config standards_root must be '{}', got '{}'",
                RUST_CARGO_STANDARDS_ROOT,
                config.standards_root.as_str()
            ),
        ));
    }

    Ok(())
}
