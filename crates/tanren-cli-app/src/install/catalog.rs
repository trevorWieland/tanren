//! Static install catalog for command and standards assets.

use std::collections::BTreeSet;

use crate::install::error::InstallError;
use crate::install::manifest::{
    AssetClass, InstallAssetProjection, PreservationPolicy, RepoRelativePath,
};
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

const RUST_CARGO_PROFILE_SOURCES: &[(&str, &str)] = &[
    (
        "profiles/rust-cargo/architecture/cookie-session.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/architecture/cookie-session.md"
        )),
    ),
    (
        "profiles/rust-cargo/architecture/crate-layering.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/architecture/crate-layering.md"
        )),
    ),
    (
        "profiles/rust-cargo/architecture/id-formats.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/architecture/id-formats.md"
        )),
    ),
    (
        "profiles/rust-cargo/architecture/naming-conventions.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/architecture/naming-conventions.md"
        )),
    ),
    (
        "profiles/rust-cargo/architecture/openapi-generation.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/architecture/openapi-generation.md"
        )),
    ),
    (
        "profiles/rust-cargo/architecture/secrets-handling.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/architecture/secrets-handling.md"
        )),
    ),
    (
        "profiles/rust-cargo/architecture/thin-binary-crate.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/architecture/thin-binary-crate.md"
        )),
    ),
    (
        "profiles/rust-cargo/architecture/trait-based-abstraction.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/architecture/trait-based-abstraction.md"
        )),
    ),
    (
        "profiles/rust-cargo/architecture/workspace-layout.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/architecture/workspace-layout.md"
        )),
    ),
    (
        "profiles/rust-cargo/global/address-deprecations-immediately.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/global/address-deprecations-immediately.md"
        )),
    ),
    (
        "profiles/rust-cargo/global/dependency-management.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/global/dependency-management.md"
        )),
    ),
    (
        "profiles/rust-cargo/global/just-ci-gate.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/global/just-ci-gate.md"
        )),
    ),
    (
        "profiles/rust-cargo/rust/edition-and-toolchain.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/rust/edition-and-toolchain.md"
        )),
    ),
    (
        "profiles/rust-cargo/rust/error-handling.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/rust/error-handling.md"
        )),
    ),
    (
        "profiles/rust-cargo/rust/file-and-function-limits.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/rust/file-and-function-limits.md"
        )),
    ),
    (
        "profiles/rust-cargo/rust/internal-visibility-controls.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/rust/internal-visibility-controls.md"
        )),
    ),
    (
        "profiles/rust-cargo/rust/naming-conventions.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/rust/naming-conventions.md"
        )),
    ),
    (
        "profiles/rust-cargo/rust/no-unsafe-no-debug-output.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/rust/no-unsafe-no-debug-output.md"
        )),
    ),
    (
        "profiles/rust-cargo/rust/type-safety-patterns.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/rust/type-safety-patterns.md"
        )),
    ),
    (
        "profiles/rust-cargo/rust/workspace-lints.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/rust/workspace-lints.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/bdd-wire-harness.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/bdd-wire-harness.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/behavior-inventory-and-scenario-traceability.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/behavior-inventory-and-scenario-traceability.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/coverage-as-scenario-proxy.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/coverage-as-scenario-proxy.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/gherkin-quality-rules.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/gherkin-quality-rules.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/mandatory-coverage.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/mandatory-coverage.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/mock-boundaries.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/mock-boundaries.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/mutation-strength-gate.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/mutation-strength-gate.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/nextest-configuration.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/nextest-configuration.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/no-test-skipping.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/no-test-skipping.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/test-timing-rules.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/test-timing-rules.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/test-tooling.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/test-tooling.md"
        )),
    ),
    (
        "profiles/rust-cargo/testing/three-tier-test-structure.md",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../profiles/rust-cargo/testing/three-tier-test-structure.md"
        )),
    ),
];

/// Build the static install asset catalog for a profile + integration selection.
pub fn build_install_asset_catalog(
    profile: InstallProfile,
    integrations: &BTreeSet<InstallIntegration>,
) -> Result<Vec<InstallAssetProjection>, InstallError> {
    let mut assets = Vec::new();

    for (command_name, source_path, content) in COMMAND_SOURCES {
        for integration in integrations {
            let destination = integration_destination(*integration, command_name);
            assets.push(install_asset(
                source_path,
                &destination,
                content,
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
                    content,
                    AssetClass::StandardsProfile,
                    None,
                    PreservationPolicy::PreserveUserEdits,
                )?);
            }
        }
    }

    Ok(assets)
}

fn install_asset(
    source_path: &str,
    destination_path: &str,
    content: &'static str,
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

fn integration_destination(integration: InstallIntegration, command_name: &str) -> String {
    match integration {
        InstallIntegration::Claude => format!(".claude/commands/{command_name}.md"),
        InstallIntegration::Codex => format!(".codex/skills/{command_name}.md"),
        InstallIntegration::OpenCode => format!(".opencode/commands/{command_name}.md"),
    }
}
