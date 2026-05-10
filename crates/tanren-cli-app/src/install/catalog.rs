//! Static install catalog for command and standards assets.

use std::collections::BTreeSet;

use tanren_configuration_secrets::{PROJECT_METHODOLOGY_SCHEMA_VERSION, ProjectMethodologyConfig};

use crate::install::error::InstallError;
use crate::install::manifest::{
    AssetClass, InstallAssetProjection, PROJECT_METHODOLOGY_CONFIG_REPO_PATH, PreservationPolicy,
    RepoRelativePath,
};
use crate::install::rust_cargo_profile_assets::RUST_CARGO_PROFILE_SOURCES;
use crate::install::{InstallIntegration, InstallProfile};

const COMMAND_SOURCES: &[(&str, &str, &str)] = &[
    (
        "plan-product",
        "commands/project/plan-product.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../commands/project/plan-product.md"
        )),
    ),
    (
        "identify-behaviors",
        "commands/project/identify-behaviors.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../commands/project/identify-behaviors.md"
        )),
    ),
    (
        "architect-system",
        "commands/project/architect-system.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../commands/project/architect-system.md"
        )),
    ),
    (
        "craft-roadmap",
        "commands/project/craft-roadmap.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../commands/project/craft-roadmap.md"
        )),
    ),
];

const CLAUDE_COMMAND_DESTINATION_ROOT: &str = ".claude/commands/";
const CODEX_COMMAND_DESTINATION_ROOT: &str = ".codex/skills/";
const OPENCODE_COMMAND_DESTINATION_ROOT: &str = ".opencode/commands/";
const RUST_CARGO_STANDARDS_ROOT: &str = "profiles/rust-cargo";

// Previously generated command destinations kept as trusted removal candidates.
// These paths are no longer emitted by the active catalog but may still exist in
// older manifests from earlier installer versions.
const LEGACY_TRUSTED_GENERATED_DESTINATIONS: &[&str] = &[".codex/skills/retired-command.md"];

/// Build the static install asset catalog for a profile + integration selection.
pub(super) fn build_install_asset_catalog(
    profile: InstallProfile,
    integrations: &BTreeSet<InstallIntegration>,
) -> Result<Vec<InstallAssetProjection>, InstallError> {
    let mut assets = Vec::new();
    let methodology_config = methodology_config_projection(profile)?;
    assets.push(install_asset(
        PROJECT_METHODOLOGY_CONFIG_REPO_PATH,
        PROJECT_METHODOLOGY_CONFIG_REPO_PATH,
        methodology_config,
        AssetClass::MethodologyConfig,
        None,
        PreservationPolicy::PreserveUserEdits,
    )?);

    for (command_name, source_path, content) in COMMAND_SOURCES {
        for integration in integrations {
            let destination = integration_destination(*integration, command_name);
            assets.push(install_asset(
                source_path,
                &destination,
                (*content).to_owned(),
                AssetClass::MethodologyCommand,
                Some(*integration),
                PreservationPolicy::ReplaceGenerated,
            )?);
        }
    }

    match profile {
        InstallProfile::RustCargo => {
            for (source_path, content) in RUST_CARGO_PROFILE_SOURCES {
                assets.push(install_asset(
                    source_path,
                    source_path,
                    (*content).to_owned(),
                    AssetClass::StandardsProfile,
                    None,
                    PreservationPolicy::PreserveUserEdits,
                )?);
            }
        }
    }

    Ok(assets)
}

/// Build a trusted registry of generated asset destinations across all profiles.
pub(super) fn build_trusted_generated_asset_registry()
-> Result<BTreeSet<RepoRelativePath>, InstallError> {
    let mut registry = BTreeSet::new();
    for profile in [InstallProfile::RustCargo] {
        for asset in build_install_asset_catalog(profile, &InstallIntegration::all())? {
            if asset.preservation == PreservationPolicy::ReplaceGenerated {
                registry.insert(asset.destination_path);
            }
        }
    }
    for path in LEGACY_TRUSTED_GENERATED_DESTINATIONS {
        registry.insert(RepoRelativePath::parse(path)?);
    }
    Ok(registry)
}

/// Integration destination roots for a selected install invocation.
#[must_use]
#[cfg(feature = "test-hooks")]
pub(super) fn generated_integration_destination_roots(
    integrations: &BTreeSet<InstallIntegration>,
) -> BTreeSet<&'static str> {
    integrations
        .iter()
        .map(|integration| integration_destination_root(*integration))
        .collect()
}

/// Whether a path matches a selected integration command destination layout.
#[must_use]
#[cfg(feature = "test-hooks")]
pub(super) fn is_current_generated_integration_destination(
    path: &RepoRelativePath,
    destination_roots: &BTreeSet<&'static str>,
) -> bool {
    destination_roots
        .iter()
        .any(|root| matches_generated_command_layout(path.as_str(), root))
}

fn install_asset(
    source_path: &str,
    destination_path: &str,
    content: String,
    asset_class: AssetClass,
    integration: Option<InstallIntegration>,
    preservation: PreservationPolicy,
) -> Result<InstallAssetProjection, InstallError> {
    Ok(InstallAssetProjection {
        source_path: RepoRelativePath::parse(source_path)?,
        destination_path: RepoRelativePath::parse(destination_path)?,
        content,
        asset_class,
        integration,
        preservation,
    })
}

fn methodology_config_projection(profile: InstallProfile) -> Result<String, InstallError> {
    let config = ProjectMethodologyConfig::new(
        PROJECT_METHODOLOGY_SCHEMA_VERSION,
        profile.methodology_profile(),
        standards_root(profile),
    )
    .map_err(|source| InstallError::InvalidProjectMethodologyConfig { source })?;
    config
        .to_toml()
        .map_err(|source| InstallError::InvalidProjectMethodologyConfig { source })
}

const fn standards_root(profile: InstallProfile) -> &'static str {
    match profile {
        InstallProfile::RustCargo => RUST_CARGO_STANDARDS_ROOT,
    }
}

fn integration_destination(integration: InstallIntegration, command_name: &str) -> String {
    format!(
        "{}{command_name}.md",
        integration_destination_root(integration)
    )
}

const fn integration_destination_root(integration: InstallIntegration) -> &'static str {
    match integration {
        InstallIntegration::Claude => CLAUDE_COMMAND_DESTINATION_ROOT,
        InstallIntegration::Codex => CODEX_COMMAND_DESTINATION_ROOT,
        InstallIntegration::OpenCode => OPENCODE_COMMAND_DESTINATION_ROOT,
    }
}

#[cfg(feature = "test-hooks")]
fn matches_generated_command_layout(path: &str, destination_root: &str) -> bool {
    let Some(filename) = path.strip_prefix(destination_root) else {
        return false;
    };
    if filename.is_empty() || filename.contains('/') {
        return false;
    }
    std::path::Path::new(filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        && filename != ".md"
}
