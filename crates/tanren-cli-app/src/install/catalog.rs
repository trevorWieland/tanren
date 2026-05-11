//! Static install catalog for command and standards assets.

use std::collections::BTreeSet;

use crate::install::error::InstallError;
use crate::install::manifest::{
    AssetClass, InstallAssetProjection, PreservationPolicy, RepoRelativePath,
};
use crate::install::{InstallIntegration, InstallProfile};

// ---------------------------------------------------------------------------
// Typed command entries (shared across all integrations)
// ---------------------------------------------------------------------------

/// One methodology command in the static catalog.
struct CommandEntry {
    name: &'static str,
    source_path: &'static str,
    content: &'static str,
}

const COMMAND_ENTRIES: &[CommandEntry] = &[
    CommandEntry {
        name: "plan-product",
        source_path: "commands/project/plan-product.md",
        content: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../commands/project/plan-product.md"
        )),
    },
    CommandEntry {
        name: "identify-behaviors",
        source_path: "commands/project/identify-behaviors.md",
        content: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../commands/project/identify-behaviors.md"
        )),
    },
    CommandEntry {
        name: "architect-system",
        source_path: "commands/project/architect-system.md",
        content: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../commands/project/architect-system.md"
        )),
    },
    CommandEntry {
        name: "craft-roadmap",
        source_path: "commands/project/craft-roadmap.md",
        content: include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../commands/project/craft-roadmap.md"
        )),
    },
];

// ---------------------------------------------------------------------------
// IntegrationSpec — one data block per integration
// ---------------------------------------------------------------------------

/// Typed specification for an install integration.
///
/// Adding a new `InstallIntegration` variant requires editing exactly one
/// `IntegrationSpec` data block, the enum definition, and `FromStr`.
struct IntegrationSpec {
    integration: InstallIntegration,
    destination_root: &'static str,
}

/// Integration data block for Claude.
const INTEGRATION_CLAUDE: IntegrationSpec = IntegrationSpec {
    integration: InstallIntegration::Claude,
    destination_root: ".claude/commands/",
};

/// Integration data block for Codex.
const INTEGRATION_CODEX: IntegrationSpec = IntegrationSpec {
    integration: InstallIntegration::Codex,
    destination_root: ".codex/skills/",
};

/// Integration data block for `OpenCode`.
const INTEGRATION_OPENCODE: IntegrationSpec = IntegrationSpec {
    integration: InstallIntegration::OpenCode,
    destination_root: ".opencode/commands/",
};

const ALL_INTEGRATION_SPECS: &[&IntegrationSpec] = &[
    &INTEGRATION_CLAUDE,
    &INTEGRATION_CODEX,
    &INTEGRATION_OPENCODE,
];

// ---------------------------------------------------------------------------
// ProfileSpec — one data block per profile
// ---------------------------------------------------------------------------

/// One standards entry within a profile specification.
struct StandardsEntry {
    source_path: &'static str,
    content: &'static str,
}

/// Typed specification for an install profile.
///
/// Adding a new `InstallProfile` variant requires editing exactly one
/// `ProfileSpec` data block, the enum definition, and `FromStr`.
struct ProfileSpec {
    profile: InstallProfile,
    standards_entries: &'static [StandardsEntry],
}

/// Profile data block for Rust + Cargo.
const PROFILE_RUST_CARGO: ProfileSpec = ProfileSpec {
    profile: InstallProfile::RustCargo,
    standards_entries: &[
        StandardsEntry {
            source_path: "profiles/rust-cargo/architecture/cookie-session.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/architecture/cookie-session.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/architecture/crate-layering.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/architecture/crate-layering.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/architecture/id-formats.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/architecture/id-formats.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/architecture/naming-conventions.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/architecture/naming-conventions.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/architecture/openapi-generation.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/architecture/openapi-generation.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/architecture/secrets-handling.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/architecture/secrets-handling.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/architecture/thin-binary-crate.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/architecture/thin-binary-crate.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/architecture/trait-based-abstraction.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/architecture/trait-based-abstraction.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/architecture/workspace-layout.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/architecture/workspace-layout.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/global/address-deprecations-immediately.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/global/address-deprecations-immediately.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/global/dependency-management.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/global/dependency-management.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/global/just-ci-gate.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/global/just-ci-gate.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/rust/edition-and-toolchain.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/rust/edition-and-toolchain.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/rust/error-handling.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/rust/error-handling.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/rust/file-and-function-limits.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/rust/file-and-function-limits.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/rust/internal-visibility-controls.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/rust/internal-visibility-controls.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/rust/naming-conventions.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/rust/naming-conventions.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/rust/no-unsafe-no-debug-output.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/rust/no-unsafe-no-debug-output.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/rust/type-safety-patterns.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/rust/type-safety-patterns.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/rust/workspace-lints.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/rust/workspace-lints.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/bdd-wire-harness.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/bdd-wire-harness.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/behavior-inventory-and-scenario-traceability.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/behavior-inventory-and-scenario-traceability.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/coverage-as-scenario-proxy.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/coverage-as-scenario-proxy.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/gherkin-quality-rules.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/gherkin-quality-rules.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/mandatory-coverage.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/mandatory-coverage.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/mock-boundaries.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/mock-boundaries.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/mutation-strength-gate.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/mutation-strength-gate.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/nextest-configuration.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/nextest-configuration.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/no-test-skipping.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/no-test-skipping.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/test-timing-rules.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/test-timing-rules.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/test-tooling.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/test-tooling.md"
            )),
        },
        StandardsEntry {
            source_path: "profiles/rust-cargo/testing/three-tier-test-structure.md",
            content: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../profiles/rust-cargo/testing/three-tier-test-structure.md"
            )),
        },
    ],
};

const ALL_PROFILE_SPECS: &[&ProfileSpec] = &[&PROFILE_RUST_CARGO];

// Previously generated command destinations kept as trusted removal candidates.
// These paths are no longer emitted by the active catalog but may still exist in
// older manifests from earlier installer versions.
const LEGACY_TRUSTED_GENERATED_DESTINATIONS: &[&str] = &[".codex/skills/retired-command.md"];

// ---------------------------------------------------------------------------
// Catalog builder
// ---------------------------------------------------------------------------

/// Build the static install asset catalog for a profile + integration selection.
pub(super) fn build_install_asset_catalog(
    profile: InstallProfile,
    integrations: &BTreeSet<InstallIntegration>,
) -> Result<Vec<InstallAssetProjection>, InstallError> {
    let mut assets = Vec::new();

    let profile_spec = find_profile_spec(profile)?;

    for command in COMMAND_ENTRIES {
        for integration in integrations {
            let spec = find_integration_spec(*integration);
            let destination = format!("{}{}.md", spec.destination_root, command.name);
            assets.push(install_asset(
                command.source_path,
                &destination,
                command.content,
                AssetClass::MethodologyCommand,
                Some(*integration),
                PreservationPolicy::ReplaceGenerated,
            )?);
        }
    }

    for entry in profile_spec.standards_entries {
        assets.push(install_asset(
            entry.source_path,
            entry.source_path,
            entry.content,
            AssetClass::StandardsProfile,
            None,
            PreservationPolicy::PreserveUserEdits,
        )?);
    }

    Ok(assets)
}

/// Build a trusted registry of generated asset destinations across all profiles.
pub(super) fn build_trusted_generated_asset_registry()
-> Result<BTreeSet<RepoRelativePath>, InstallError> {
    let mut registry = BTreeSet::new();
    for spec in ALL_PROFILE_SPECS {
        for asset in build_install_asset_catalog(spec.profile, &InstallIntegration::all())? {
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
        .map(|integration| find_integration_spec(*integration).destination_root)
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

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn find_profile_spec(profile: InstallProfile) -> Result<&'static ProfileSpec, InstallError> {
    ALL_PROFILE_SPECS
        .iter()
        .find(|spec| spec.profile == profile)
        .copied()
        .ok_or_else(|| InstallError::UnsupportedProfile {
            name: profile.as_str().to_owned(),
        })
}

fn find_integration_spec(integration: InstallIntegration) -> &'static IntegrationSpec {
    ALL_INTEGRATION_SPECS
        .iter()
        .find(|spec| spec.integration == integration)
        .expect("every InstallIntegration variant must have a matching IntegrationSpec")
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
