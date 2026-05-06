//! Reusable BDD fixture for asset upgrade scenarios (R-0026).
//!
//! [`UpgradeFixture`] creates a temporary repository root with a
//! `.tanren/asset-manifest` that records a prior installation version.
//! The fixture includes Tanren-generated assets that differ from the
//! current embedded bundle (producing `Create`/`Update`/`Remove`
//! actions on preview), user-owned assets that the upgrade planner
//! must preserve, and a fixtured migration concern (`HashMismatch`
//! on a Tanren-owned asset whose manifest hash diverges from the
//! on-disk content).

use std::fmt::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tanren_contract::{AssetEntry, AssetManifest, AssetOwnership, MANIFEST_FORMAT_VERSION};

const FIXTURE_SOURCE_VERSION: &str = "0.1.0-fixture";

/// A disposable repository root pre-populated with an installed manifest,
/// generated assets, user-owned standards, and a fixtured migration
/// concern. Drop removes the temporary directory.
///
/// # Layout
///
/// - `.tanren/asset-manifest` — versioned manifest at `0.1.0-fixture`
/// - `.tanren/config.toml` — Tanren-owned, old content; manifest
///   records a stale hash so the preview produces an `Update` action
///   and a `HashMismatch` concern
/// - `commands/check.md` — Tanren-owned, identical to the current
///   embedded bundle (no action)
/// - `commands/retired.md` — Tanren-owned, absent from the current
///   bundle (`Remove` action, `RemovedAsset` concern)
/// - `standards/team-policy.md` — User-owned (`Preserve` action)
///
/// `commands/build.md` is in the current bundle but NOT in the
/// installed manifest, producing a `Create` action.
pub struct UpgradeFixture {
    root: PathBuf,
}

impl UpgradeFixture {
    /// Create and populate a temporary repository root suitable for
    /// upgrade preview / apply BDD scenarios.
    pub fn install() -> Self {
        let root = Self::create_root_dir();
        Self::write_structure(&root);
        Self { root }
    }

    /// The temporary repository root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn create_root_dir() -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "tanren-upgrade-fixture-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&p).expect("create fixture root");
        p
    }

    fn write_structure(root: &Path) {
        let tanren_dir = root.join(".tanren");
        let commands_dir = root.join("commands");
        let standards_dir = root.join("standards");
        std::fs::create_dir_all(&tanren_dir).expect("create .tanren");
        std::fs::create_dir_all(&commands_dir).expect("create commands");
        std::fs::create_dir_all(&standards_dir).expect("create standards");

        let config_old = b"# Tanren configuration (old)\n";
        std::fs::write(tanren_dir.join("config.toml"), config_old).expect("write config");

        let check_content = b"# Check command documentation\n";
        std::fs::write(commands_dir.join("check.md"), check_content).expect("write check");
        let check_hash = compute_hash(check_content);

        let retired_content = b"# Retired command documentation\n";
        std::fs::write(commands_dir.join("retired.md"), retired_content).expect("write retired");
        let retired_hash = compute_hash(retired_content);

        let user_content = b"# Team policy\n";
        std::fs::write(standards_dir.join("team-policy.md"), user_content).expect("write standard");
        let user_hash = compute_hash(user_content);

        let manifest = AssetManifest {
            version: MANIFEST_FORMAT_VERSION,
            source_version: FIXTURE_SOURCE_VERSION.to_owned(),
            assets: vec![
                AssetEntry {
                    path: PathBuf::from(".tanren/config.toml"),
                    hash: "sha256:0000000000000000abcdef0123456789".to_owned(),
                    ownership: AssetOwnership::Tanren,
                    installed_from: FIXTURE_SOURCE_VERSION.to_owned(),
                },
                AssetEntry {
                    path: PathBuf::from("commands/check.md"),
                    hash: check_hash,
                    ownership: AssetOwnership::Tanren,
                    installed_from: FIXTURE_SOURCE_VERSION.to_owned(),
                },
                AssetEntry {
                    path: PathBuf::from("commands/retired.md"),
                    hash: retired_hash,
                    ownership: AssetOwnership::Tanren,
                    installed_from: FIXTURE_SOURCE_VERSION.to_owned(),
                },
                AssetEntry {
                    path: PathBuf::from("standards/team-policy.md"),
                    hash: user_hash,
                    ownership: AssetOwnership::User,
                    installed_from: FIXTURE_SOURCE_VERSION.to_owned(),
                },
            ],
        };

        let manifest_toml = toml::to_string_pretty(&manifest).expect("serialize manifest");
        std::fs::write(tanren_dir.join("asset-manifest"), manifest_toml).expect("write manifest");
    }
}

impl std::fmt::Debug for UpgradeFixture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UpgradeFixture")
            .field("root", &self.root)
            .finish()
    }
}

impl Drop for UpgradeFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn compute_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(result.len() * 2);
    for byte in result {
        let _ = write!(hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
}
