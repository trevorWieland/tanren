use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Json;
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tanren_delivery::install::contract::append_stale_generated_manifest_entry;
use tanren_delivery::install::{RepoRelativePath, apply_install, run_upgrade_witness, sha256_hex};
use tokio::sync::Mutex;

use self::snapshot::{RepositorySnapshot, capture_scoped_snapshot};
use super::TestHooksState;

mod snapshot;

const INSTALL_MANIFEST_PATH: &str = ".tanren/install-manifest.toml";
const LEGACY_MIGRATION_CONTENT: &str = "legacy standards asset requiring migration";

static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(crate) struct UpgradeFixtureHarness {
    workspace_root: PathBuf,
    state: Arc<Mutex<UpgradeFixtureState>>,
}

impl UpgradeFixtureHarness {
    pub(crate) fn new() -> Self {
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            workspace_root,
            state: Arc::new(Mutex::new(UpgradeFixtureState::default())),
        }
    }
}

#[derive(Debug, Default)]
struct UpgradeFixtureState {
    repository_root: Option<PathBuf>,
    labeled_snapshots: BTreeMap<String, RepositorySnapshot>,
    file_baselines: BTreeMap<String, Vec<u8>>,
    tracked_snapshot_paths: BTreeSet<String>,
    snapshot_before_last_run: Option<RepositorySnapshot>,
    last_run: Option<CommandResult>,
}

#[derive(Debug, Clone, Serialize)]
struct CommandResult {
    stdout: String,
    status: i32,
    success: bool,
}

#[derive(Debug, Deserialize)]
struct FileWriteBody {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct FilePathBody {
    path: String,
}

#[derive(Debug, Deserialize)]
struct InstallSeedBody {
    snapshot_label: String,
    profile: String,
    integrations: String,
}

#[derive(Debug, Deserialize)]
struct SnapshotBody {
    label: String,
}

pub(crate) async fn upgrade_fixture_action_route(
    State(state): State<TestHooksState>,
    AxumPath(action): AxumPath<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let mut fixture_state = state.upgrade_fixture.state.lock().await;
    dispatch_action(
        action.as_str(),
        body,
        &state.upgrade_fixture.workspace_root,
        &mut fixture_state,
    )
}

fn dispatch_action(
    action: &str,
    body: Value,
    workspace_root: &Path,
    fixture_state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    match action {
        "reset" => action_reset(fixture_state),
        "write-file" => action_write_file(body, fixture_state),
        "record-baseline" => action_record_baseline(body, fixture_state),
        "seed-install" => action_seed_install(body, workspace_root, fixture_state),
        "mark-legacy-migration-concern" => action_mark_legacy_migration(body, fixture_state),
        "capture-snapshot" => action_capture_snapshot(body, workspace_root, fixture_state),
        "run-upgrade-preview" => action_run_upgrade(false, workspace_root, fixture_state),
        "run-upgrade-apply" => action_run_upgrade(true, workspace_root, fixture_state),
        "assert-no-writes" => action_assert_no_writes(workspace_root, fixture_state),
        "assert-matches-snapshot" => {
            action_assert_matches_snapshot(body, workspace_root, fixture_state)
        }
        "assert-preserves-baseline" => action_assert_preserves_baseline(body, fixture_state),
        "assert-replaced-from-baseline" => {
            action_assert_replaced_from_baseline(body, fixture_state)
        }
        "assert-file-missing" => action_assert_file_missing(body, fixture_state),
        "last-run" => action_last_run(fixture_state),
        _ => Err((
            StatusCode::NOT_FOUND,
            format!("unknown upgrade-fixture action '{action}'"),
        )),
    }
}

fn action_reset(state: &mut UpgradeFixtureState) -> Result<Json<Value>, (StatusCode, String)> {
    reset_fixture(state)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_write_file(
    body: Value,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let payload: FileWriteBody = decode_payload(body)?;
    let root = require_repository_root(state)?.to_path_buf();
    let relative = parse_repository_path(&payload.path)?;
    state
        .tracked_snapshot_paths
        .insert(relative.as_str().to_owned());
    let absolute = root.join(relative.as_str());
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| io_error(&source, parent, "create parent directories"))?;
    }
    fs::write(&absolute, payload.content.as_bytes())
        .map_err(|source| io_error(&source, &absolute, "write repository fixture file"))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_record_baseline(
    body: Value,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let payload: FilePathBody = decode_payload(body)?;
    let root = require_repository_root(state)?.to_path_buf();
    let relative = parse_repository_path(&payload.path)?;
    state
        .tracked_snapshot_paths
        .insert(relative.as_str().to_owned());
    let absolute = root.join(relative.as_str());
    let baseline = fs::read(&absolute).map_err(|source| {
        io_error(
            &source,
            &absolute,
            "read repository fixture file for baseline",
        )
    })?;
    state
        .file_baselines
        .insert(relative.as_str().to_owned(), baseline);
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_seed_install(
    body: Value,
    workspace_root: &Path,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let payload: InstallSeedBody = decode_payload(body)?;
    let root = require_repository_root(state)?;
    apply_install(
        root,
        payload.profile.as_str(),
        Some(payload.integrations.as_str()),
    )
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    capture_labeled_snapshot(workspace_root, state, payload.snapshot_label.as_str())?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_mark_legacy_migration(
    body: Value,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let payload: FilePathBody = decode_payload(body)?;
    let root = require_repository_root(state)?.to_path_buf();
    let relative = parse_repository_path(&payload.path)?;
    state
        .tracked_snapshot_paths
        .insert(relative.as_str().to_owned());
    let absolute = root.join(relative.as_str());
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| io_error(&source, parent, "create parent directories"))?;
    }
    fs::write(&absolute, LEGACY_MIGRATION_CONTENT.as_bytes())
        .map_err(|source| io_error(&source, &absolute, "write legacy migration fixture file"))?;

    let manifest_path = root.join(INSTALL_MANIFEST_PATH);
    let mut manifest = fs::read_to_string(&manifest_path)
        .map_err(|source| io_error(&source, &manifest_path, "read install manifest"))?;
    let stale_hash = sha256_hex(LEGACY_MIGRATION_CONTENT.as_bytes()).to_string();
    append_stale_generated_manifest_entry(&mut manifest, &relative, stale_hash.as_str())
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    fs::write(&manifest_path, manifest.as_bytes())
        .map_err(|source| io_error(&source, &manifest_path, "write install manifest"))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_capture_snapshot(
    body: Value,
    workspace_root: &Path,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let payload: SnapshotBody = decode_payload(body)?;
    capture_labeled_snapshot(workspace_root, state, payload.label.as_str())?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_run_upgrade(
    confirm: bool,
    workspace_root: &Path,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let root = require_repository_root(state)?;
    let before = capture_scoped_snapshot(root, workspace_root, &state.tracked_snapshot_paths)?;
    let run = run_upgrade_witness(root, confirm)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let result = CommandResult {
        stdout: run.stdout(),
        status: 0,
        success: true,
    };
    state.snapshot_before_last_run = Some(before);
    state.last_run = Some(result.clone());
    Ok(Json(serde_json::json!({
        "ok": true,
        "stdout": result.stdout,
        "status": result.status,
        "success": result.success
    })))
}

fn action_assert_no_writes(
    workspace_root: &Path,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let root = require_repository_root(state)?;
    let before = state.snapshot_before_last_run.clone().ok_or((
        StatusCode::BAD_REQUEST,
        "missing snapshot before last command run".to_owned(),
    ))?;
    let after = capture_scoped_snapshot(root, workspace_root, &state.tracked_snapshot_paths)?;
    if after != before {
        return Err((
            StatusCode::CONFLICT,
            "repository changed but expected no writes".to_owned(),
        ));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_assert_matches_snapshot(
    body: Value,
    workspace_root: &Path,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let payload: SnapshotBody = decode_payload(body)?;
    let root = require_repository_root(state)?;
    let expected = state
        .labeled_snapshots
        .get(payload.label.as_str())
        .cloned()
        .ok_or((
            StatusCode::BAD_REQUEST,
            format!("missing labeled snapshot '{}'", payload.label),
        ))?;
    let actual = capture_scoped_snapshot(root, workspace_root, &state.tracked_snapshot_paths)?;
    if actual != expected {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "repository does not match labeled snapshot '{}'",
                payload.label
            ),
        ));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_assert_preserves_baseline(
    body: Value,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let payload: FilePathBody = decode_payload(body)?;
    let root = require_repository_root(state)?.to_path_buf();
    let relative = parse_repository_path(&payload.path)?;
    let baseline = state
        .file_baselines
        .get(relative.as_str())
        .cloned()
        .ok_or((
            StatusCode::BAD_REQUEST,
            format!("missing baseline for '{}'", relative.as_str()),
        ))?;
    let absolute = root.join(relative.as_str());
    let current = fs::read(&absolute)
        .map_err(|source| io_error(&source, &absolute, "read repository fixture file"))?;
    if current != baseline {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "expected '{}' to preserve baseline content",
                relative.as_str()
            ),
        ));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_assert_replaced_from_baseline(
    body: Value,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let payload: FilePathBody = decode_payload(body)?;
    let root = require_repository_root(state)?.to_path_buf();
    let relative = parse_repository_path(&payload.path)?;
    let baseline = state
        .file_baselines
        .get(relative.as_str())
        .cloned()
        .ok_or((
            StatusCode::BAD_REQUEST,
            format!("missing baseline for '{}'", relative.as_str()),
        ))?;
    let absolute = root.join(relative.as_str());
    let current = fs::read(&absolute)
        .map_err(|source| io_error(&source, &absolute, "read repository fixture file"))?;
    if current == baseline {
        return Err((
            StatusCode::CONFLICT,
            format!("expected '{}' to differ from baseline", relative.as_str()),
        ));
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn action_assert_file_missing(
    body: Value,
    state: &mut UpgradeFixtureState,
) -> Result<Json<Value>, (StatusCode, String)> {
    let payload: FilePathBody = decode_payload(body)?;
    let root = require_repository_root(state)?.to_path_buf();
    let relative = parse_repository_path(&payload.path)?;
    let absolute = root.join(relative.as_str());
    match fs::metadata(&absolute) {
        Ok(_) => Err((
            StatusCode::CONFLICT,
            format!("expected '{}' to be absent", relative.as_str()),
        )),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(Json(serde_json::json!({ "ok": true })))
        }
        Err(source) => Err(io_error(
            &source,
            &absolute,
            "inspect repository fixture path",
        )),
    }
}

fn action_last_run(state: &UpgradeFixtureState) -> Result<Json<Value>, (StatusCode, String)> {
    let run = state.last_run.clone().ok_or((
        StatusCode::BAD_REQUEST,
        "upgrade command has not been executed".to_owned(),
    ))?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "stdout": run.stdout,
        "status": run.status,
        "success": run.success
    })))
}

fn reset_fixture(state: &mut UpgradeFixtureState) -> Result<(), (StatusCode, String)> {
    if let Some(path) = state.repository_root.take() {
        let _ = fs::remove_dir_all(path);
    }
    let root = create_fixture_root()?;
    state.repository_root = Some(root);
    state.labeled_snapshots.clear();
    state.file_baselines.clear();
    state.tracked_snapshot_paths.clear();
    state.snapshot_before_last_run = None;
    state.last_run = None;
    Ok(())
}

fn create_fixture_root() -> Result<PathBuf, (StatusCode, String)> {
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let sequence = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "tanren-web-bdd-install-{}-{sequence}-{now_nanos}",
        std::process::id(),
    ));
    fs::create_dir_all(&path).map_err(|source| io_error(&source, &path, "create fixture root"))?;
    Ok(path)
}

fn capture_labeled_snapshot(
    workspace_root: &Path,
    state: &mut UpgradeFixtureState,
    label: &str,
) -> Result<(), (StatusCode, String)> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "snapshot label cannot be empty".to_owned(),
        ));
    }
    let root = require_repository_root(state)?;
    let snapshot = capture_scoped_snapshot(root, workspace_root, &state.tracked_snapshot_paths)?;
    state.labeled_snapshots.insert(trimmed.to_owned(), snapshot);
    Ok(())
}

fn require_repository_root(state: &UpgradeFixtureState) -> Result<&Path, (StatusCode, String)> {
    state.repository_root.as_deref().ok_or((
        StatusCode::BAD_REQUEST,
        "repository fixture is not initialized".to_owned(),
    ))
}

fn parse_repository_path(raw: &str) -> Result<RepoRelativePath, (StatusCode, String)> {
    RepoRelativePath::parse(raw.trim()).map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
}

fn decode_payload<T: for<'de> Deserialize<'de>>(body: Value) -> Result<T, (StatusCode, String)> {
    serde_json::from_value(body).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid action payload: {err}"),
        )
    })
}

fn io_error(source: &std::io::Error, path: &Path, action: &'static str) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("failed to {action} at '{}': {source}", path.display()),
    )
}
