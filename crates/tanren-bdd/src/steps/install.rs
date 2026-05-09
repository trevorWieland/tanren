//! Install-flow step definitions for B-0068 / B-0070.
//!
//! The steps execute the real `tanren-cli` binary against a per-scenario
//! temporary repository fixture. No installer internals are called directly.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cucumber::{given, then, when};
use tokio::process::Command;

use tanren_testkit::locate_workspace_binary;

use crate::TanrenWorld;
use crate::steps::install_helpers;
use crate::steps::install_helpers::RepositoryRelativePath;

static SCENARIO_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Per-scenario install fixture state.
#[derive(Debug)]
pub(crate) struct InstallContext {
    repository_root: PathBuf,
    baselines: BTreeMap<RepositoryRelativePath, Vec<u8>>,
    snapshot_before_last_run: Option<RepositorySnapshot>,
    last_run: Option<InstallCommandOutcome>,
}

impl InstallContext {
    #[must_use]
    pub(crate) fn new() -> Self {
        let repository_root = scenario_repository_root();
        fs::create_dir_all(&repository_root)
            .expect("create install scenario repository fixture directory");
        Self {
            repository_root,
            baselines: BTreeMap::new(),
            snapshot_before_last_run: None,
            last_run: None,
        }
    }

    async fn run_install(&mut self, profile: &str, integrations: Option<&str>) {
        let before = RepositorySnapshot::capture(&self.repository_root);
        let binary = locate_workspace_binary("tanren-cli")
            .expect("locate tanren-cli binary for BDD install steps");

        let mut command = Command::new(binary);
        command
            .arg("install")
            .arg("--repo")
            .arg(&self.repository_root)
            .arg("--profile")
            .arg(profile);
        if let Some(selected) = integrations {
            command.arg("--integrations").arg(selected);
        }

        let output = command
            .output()
            .await
            .expect("spawn tanren-cli install subprocess");
        self.snapshot_before_last_run = Some(before);
        self.last_run = Some(InstallCommandOutcome::from(output));
    }

    fn assert_success(&self) {
        let run = self.require_last_run();
        assert!(
            run.success,
            "expected install command to succeed; status={:?}\nstdout:\n{}\nstderr:\n{}",
            run.status_code, run.stdout, run.stderr,
        );
    }

    fn assert_nonzero(&self) {
        let run = self.require_last_run();
        assert!(
            !run.success,
            "expected install command to fail with a non-zero exit; stdout:\n{}\nstderr:\n{}",
            run.stdout, run.stderr,
        );
    }

    fn assert_summary_output(&self) {
        let run = self.require_last_run();
        assert!(
            run.stdout.contains("status=ok command=install"),
            "expected status line in stdout; got:\n{}",
            run.stdout,
        );
        for field in [
            "created=",
            "updated=",
            "removed=",
            "restored=",
            "preserved=",
        ] {
            assert!(
                run.stdout.contains(field),
                "expected summary field `{field}` in stdout; got:\n{}",
                run.stdout,
            );
        }
        for section in [
            "paths created=[",
            "updated=[",
            "removed=[",
            "restored=[",
            "preserved=[",
        ] {
            assert!(
                run.stdout.contains(section),
                "expected path section `{section}` in stdout; got:\n{}",
                run.stdout,
            );
        }
    }

    fn assert_validation_failure_output(&self) {
        let run = self.require_last_run();
        assert!(
            run.stderr.contains("error: validation_failed"),
            "expected validation failure in stderr; got:\n{}",
            run.stderr,
        );
    }

    fn assert_no_writes_since_last_run(&self) {
        let before = self
            .snapshot_before_last_run
            .as_ref()
            .expect("install command must run before no-write assertion");
        let after = RepositorySnapshot::capture(&self.repository_root);
        assert_eq!(
            &after, before,
            "expected repository fixture to remain unchanged after command"
        );
    }

    pub(crate) fn assert_stderr_contains(&self, expected: &str) {
        let run = self.require_last_run();
        assert!(
            run.stderr.contains(expected),
            "expected stderr to contain `{expected}`; got:\n{}",
            run.stderr,
        );
    }

    fn write_fixture_file(&mut self, relative_path: &RepositoryRelativePath, content: String) {
        let absolute = self.repository_path(relative_path.as_str());
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent).expect("create parent directories in repository fixture");
        }
        fs::write(&absolute, content).expect("write repository fixture file");
    }

    fn record_baseline(&mut self, relative_path: RepositoryRelativePath) {
        let absolute = self.repository_path(relative_path.as_str());
        let bytes = fs::read(&absolute).expect("read repository fixture file for baseline");
        self.baselines.insert(relative_path, bytes);
    }

    fn assert_file_exists(&self, relative_path: &RepositoryRelativePath) {
        let absolute = self.repository_path(relative_path.as_str());
        assert!(
            absolute.exists(),
            "expected repository file to exist: {}",
            absolute.display()
        );
    }

    fn assert_file_absent(&self, relative_path: &RepositoryRelativePath) {
        let absolute = self.repository_path(relative_path.as_str());
        assert!(
            !absolute.exists(),
            "expected repository file to be absent: {}",
            absolute.display()
        );
    }

    fn assert_exact_file_content(&self, relative_path: &RepositoryRelativePath, expected: String) {
        let absolute = self.repository_path(relative_path.as_str());
        let bytes = fs::read(&absolute).expect("read repository fixture file");
        assert_eq!(
            bytes,
            expected.into_bytes(),
            "unexpected repository file content for {}",
            absolute.display(),
        );
    }

    fn assert_file_content_preserved(&self, relative_path: &RepositoryRelativePath) {
        let baseline = self
            .baselines
            .get(relative_path)
            .expect("baseline must be recorded before preservation assertion");
        let absolute = self.repository_path(relative_path.as_str());
        let bytes = fs::read(&absolute).expect("read repository fixture file");
        assert_eq!(
            &bytes,
            baseline,
            "expected repository file content to be preserved for {}",
            absolute.display(),
        );
    }

    fn assert_file_content_replaced(&self, relative_path: &RepositoryRelativePath) {
        let baseline = self
            .baselines
            .get(relative_path)
            .expect("baseline must be recorded before replacement assertion");
        let absolute = self.repository_path(relative_path.as_str());
        let bytes = fs::read(&absolute).expect("read repository fixture file");
        assert_ne!(
            &bytes,
            baseline,
            "expected generated repository file to be replaced for {}",
            absolute.display(),
        );
    }

    fn assert_rust_cargo_default_assets_installed(&self) {
        install_helpers::assert_rust_cargo_default_assets_installed(&self.repository_root);
    }

    pub(crate) fn assert_rust_cargo_standards_installed(&self) {
        install_helpers::assert_rust_cargo_standards_installed(&self.repository_root);
    }

    pub(crate) fn assert_selected_integration_command_assets(&self, integrations: &str) {
        let selected = integrations
            .split(',')
            .map(str::trim)
            .filter(|integration| !integration.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        install_helpers::assert_selected_integration_command_assets(
            &self.repository_root,
            &selected,
        );
    }

    fn assert_manifest_rust_cargo_defaults(&self) {
        install_helpers::assert_manifest_rust_cargo_defaults(&self.repository_root);
    }

    fn inject_manifest_stale_generated_entry(&mut self, relative_path: &RepositoryRelativePath) {
        self.assert_file_exists(relative_path);

        let manifest_path = self.repository_path(".tanren/install-manifest.toml");
        let mut manifest = fs::read_to_string(&manifest_path).expect("read install manifest");
        let path_line = format!("path = \"{}\"", relative_path.as_str());
        assert!(
            !manifest.contains(&path_line),
            "expected stale path to be absent before manifest injection: {}",
            relative_path.as_str(),
        );
        install_helpers::append_stale_generated_manifest_entry(&mut manifest, relative_path);
        fs::write(&manifest_path, manifest).expect("write install manifest with stale entry");
    }

    fn delete_fixture_file(&mut self, relative_path: &RepositoryRelativePath) {
        let absolute = self.repository_path(relative_path.as_str());
        assert!(
            absolute.exists(),
            "expected repository file to exist before deletion: {}",
            absolute.display()
        );
        fs::remove_file(&absolute).expect("delete repository fixture file");
    }

    fn repository_path(&self, relative_path: &str) -> PathBuf {
        install_helpers::validate_relative_path(relative_path);
        self.repository_root.join(relative_path)
    }

    fn require_last_run(&self) -> &InstallCommandOutcome {
        self.last_run
            .as_ref()
            .expect("install command has not been executed yet")
    }
}

impl Default for InstallContext {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for InstallContext {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.repository_root);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstallCommandOutcome {
    status_code: Option<i32>,
    success: bool,
    stdout: String,
    stderr: String,
}

impl From<std::process::Output> for InstallCommandOutcome {
    fn from(output: std::process::Output) -> Self {
        Self {
            status_code: output.status.code(),
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepositorySnapshot {
    files: BTreeMap<String, Vec<u8>>,
}

impl RepositorySnapshot {
    fn capture(root: &Path) -> Self {
        let mut files = BTreeMap::new();
        collect_files(root, root, &mut files);
        Self { files }
    }
}

fn collect_files(root: &Path, cursor: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    let entries = fs::read_dir(cursor).expect("read repository fixture directory");
    for entry in entries {
        let entry = entry.expect("inspect repository fixture directory entry");
        let path = entry.path();
        let file_type = entry
            .file_type()
            .expect("inspect repository fixture file type");
        if file_type.is_dir() {
            collect_files(root, &path, out);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .expect("repository fixture path must be rooted under fixture root");
        let relative = relative.to_string_lossy().replace('\\', "/");
        let bytes = fs::read(&path).expect("read repository fixture file bytes");
        out.insert(relative, bytes);
    }
}

fn scenario_repository_root() -> PathBuf {
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = SCENARIO_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "tanren-bdd-install-{}-{sequence}-{now_nanos}",
        std::process::id(),
    ))
}

#[given(expr = "a clean repository fixture")]
#[given(expr = "a clean install repository fixture")]
fn given_clean_repository_fixture(world: &mut TanrenWorld) {
    world.install = Some(InstallContext::new());
}

#[given(expr = "repository file {string} contains {string}")]
fn given_repository_file_contains(world: &mut TanrenWorld, path: String, content: String) {
    let ctx = world.ensure_install_ctx();
    let relative_path = RepositoryRelativePath::parse(path);
    ctx.write_fixture_file(&relative_path, content);
}

#[given(expr = "repository file {string} baseline is recorded")]
fn given_repository_file_baseline(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    ctx.record_baseline(RepositoryRelativePath::parse(path));
}

#[given(expr = "previous install manifest tracks stale generated file {string}")]
fn given_previous_manifest_tracks_stale_generated_file(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    let relative_path = RepositoryRelativePath::parse(path);
    ctx.inject_manifest_stale_generated_entry(&relative_path);
}

#[given(expr = "repository file {string} is deleted from the repository fixture")]
fn given_repository_file_deleted(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    let relative_path = RepositoryRelativePath::parse(path);
    ctx.delete_fixture_file(&relative_path);
}

#[when(expr = "tanren-cli install runs with profile {string}")]
async fn when_install_runs_with_profile(world: &mut TanrenWorld, profile: String) {
    let ctx = world.ensure_install_ctx();
    ctx.run_install(&profile, None).await;
}

#[when(expr = "tanren-cli install runs with profile {string} and integrations {string}")]
async fn when_install_runs_with_profile_and_integrations(
    world: &mut TanrenWorld,
    profile: String,
    integrations: String,
) {
    let ctx = world.ensure_install_ctx();
    ctx.run_install(&profile, Some(integrations.as_str())).await;
}

#[then(expr = "the install command succeeds")]
#[then(expr = "install command succeeds")]
fn then_install_command_succeeds(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_success();
}

#[then(expr = "the install command exits nonzero")]
#[then(expr = "install command exits nonzero")]
fn then_install_command_exits_nonzero(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_nonzero();
}

#[then(
    expr = "the install output reports created, updated, removed, restored, and preserved summaries"
)]
fn then_install_output_reports_summaries(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_summary_output();
}

#[then(expr = "rust-cargo defaults install all methodology command assets and standards files")]
fn then_rust_cargo_defaults_install_all_assets(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_rust_cargo_default_assets_installed();
}

#[then(expr = "the install manifest records the rust-cargo profile and default integrations")]
fn then_install_manifest_records_rust_cargo_defaults(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_manifest_rust_cargo_defaults();
}

#[then(expr = "the install output reports a validation failure")]
fn then_install_output_reports_validation_failure(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_validation_failure_output();
}

#[then(expr = "no files are written in the repository fixture")]
#[then(expr = "no files are written in repository fixture")]
fn then_no_files_are_written(world: &mut TanrenWorld) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_no_writes_since_last_run();
}

#[then(expr = "repository file {string} exists")]
fn then_repository_file_exists(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_file_exists(&RepositoryRelativePath::parse(path));
}

#[then(expr = "repository file {string} does not exist")]
fn then_repository_file_absent(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_file_absent(&RepositoryRelativePath::parse(path));
}

#[then(expr = "repository file {string} contains {string}")]
fn then_repository_file_contains(world: &mut TanrenWorld, path: String, content: String) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_exact_file_content(&RepositoryRelativePath::parse(path), content);
}

#[then(expr = "repository file {string} preserves its baseline content")]
fn then_repository_file_preserved(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_file_content_preserved(&RepositoryRelativePath::parse(path));
}

#[then(expr = "repository file {string} is replaced from its baseline content")]
fn then_repository_file_replaced(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_file_content_replaced(&RepositoryRelativePath::parse(path));
}

#[then(expr = "stale generated file {string} is removed")]
fn then_stale_generated_file_removed(world: &mut TanrenWorld, path: String) {
    let ctx = world.ensure_install_ctx();
    ctx.assert_file_absent(&RepositoryRelativePath::parse(path));
}
