#![cfg(all(test, unix))]

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tanren_cli_app::install::{
    InstallError, InstallIntegration, InstallProfile, apply_install,
    manifest::{
        AssetClass, InstallManifest, ManifestEntry, PreservationPolicy, RepoRelativePath,
        sha256_hex,
    },
    plan::build_install_plan,
    writer::apply_install_plan,
};

static TEST_REPO_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[test]
fn apply_rejects_symlink_swapped_in_after_planning_for_write()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestRepo::new("writer-write")?;
    let integrations = selected_integrations();

    let plan = build_install_plan(fixture.path(), InstallProfile::RustCargo, &integrations)?;

    let attacked_prefix = ".codex/skills/";
    if !plan
        .writes()
        .iter()
        .any(|write| write.path().as_str().starts_with(attacked_prefix))
    {
        return Err(test_error("expected planned codex command writes"));
    }

    fs::create_dir_all(fixture.path().join(".codex"))?;
    let external = fixture.path().join("outside-write-target");
    fs::create_dir_all(&external)?;
    symlink(external, fixture.path().join(".codex/skills"))?;

    let Err(err) = apply_install_plan(&plan) else {
        return Err(test_error("expected install apply to fail"));
    };

    let (path, message) = assert_unsafe_symlink_error(&err);
    assert!(path.starts_with(attacked_prefix));
    assert!(message.contains("symbolic link"));
    Ok(())
}

#[test]
fn apply_rejects_symlink_swapped_in_after_planning_for_removal()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = TestRepo::new("writer-removal")?;
    let integrations = selected_integrations();

    apply_install(fixture.path(), "rust-cargo", Some("codex"))?;

    let stale_path = RepoRelativePath::parse(".codex/skills/retired-command.md")?;
    let stale_absolute = fixture.path().join(stale_path.as_str());
    let stale_content = "stale generated codex command";
    fs::write(&stale_absolute, stale_content)?;

    inject_stale_manifest_entry(
        fixture.path(),
        &stale_path,
        sha256_hex(stale_content.as_bytes()),
    )?;

    let plan = build_install_plan(fixture.path(), InstallProfile::RustCargo, &integrations)?;
    if !plan
        .removals()
        .iter()
        .any(|removal| removal.path().as_str() == stale_path.as_str())
    {
        return Err(test_error(
            "expected stale generated file to be scheduled for removal",
        ));
    }

    fs::remove_file(&stale_absolute)?;
    let external_target = fixture.path().join("outside-remove-target.md");
    fs::write(&external_target, "external")?;
    symlink(&external_target, &stale_absolute)?;

    let Err(err) = apply_install_plan(&plan) else {
        return Err(test_error("expected install apply to fail"));
    };

    let (path, message) = assert_unsafe_symlink_error(&err);
    assert_eq!(path, stale_path.as_str());
    assert!(message.contains("symbolic link"));
    Ok(())
}

fn selected_integrations() -> BTreeSet<InstallIntegration> {
    [InstallIntegration::Codex].into_iter().collect()
}

fn inject_stale_manifest_entry(
    repository_root: &Path,
    stale_path: &RepoRelativePath,
    content_hash: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest_path = repository_root.join(".tanren/install-manifest.toml");
    let manifest_raw = fs::read_to_string(&manifest_path)?;
    let mut manifest: InstallManifest = toml::from_str(&manifest_raw)?;
    manifest.entries.push(ManifestEntry {
        path: stale_path.clone(),
        content_hash,
        asset_class: AssetClass::MethodologyCommand,
        integration: Some(InstallIntegration::Codex),
        preservation: PreservationPolicy::ReplaceGenerated,
    });
    fs::write(&manifest_path, toml::to_string(&manifest)?)?;
    Ok(())
}

fn assert_unsafe_symlink_error(err: &InstallError) -> (&str, &str) {
    assert!(matches!(err, InstallError::UnsafeRepositoryPath { .. }));
    if let InstallError::UnsafeRepositoryPath { path, message } = err {
        (path.as_str(), message.as_str())
    } else {
        ("", "")
    }
}

fn test_error(message: &str) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::other(message.to_owned()))
}

#[derive(Debug)]
struct TestRepo {
    root: PathBuf,
}

impl TestRepo {
    fn new(label: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let sequence = TEST_REPO_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root = std::env::temp_dir().join(format!(
            "tanren-bdd-install-writer-{label}-{}-{sequence}-{now_nanos}",
            std::process::id(),
        ));
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        if self.root.exists() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
