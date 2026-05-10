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
use self::types::{FixtureAction, FixtureId, SnapshotLabel};
use super::TestHooksState;
use super::limits;

mod snapshot;
mod types;

const INSTALL_MANIFEST_PATH: &str = ".tanren/install-manifest.toml";
const LEGACY_MIGRATION_CONTENT: &str = "legacy standards asset requiring migration";
static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub(crate) struct UpgradeFixtureHarness {
    workspace_root: PathBuf,
    state: Arc<Mutex<BTreeMap<FixtureId, UpgradeFixtureState>>>,
}

impl UpgradeFixtureHarness {
    pub(crate) fn new() -> Self {
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            workspace_root,
            state: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[derive(Debug, Default)]
struct UpgradeFixtureState {
    repository_root: Option<PathBuf>,
    labeled_snapshots: BTreeMap<SnapshotLabel, RepositorySnapshot>,
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
    AxumPath(action): AxumPath<FixtureAction>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    limits::validate_body_size(&body)?;
    let fixture_id = extract_fixture_id(&body)?;
    let workspace_root = state.upgrade_fixture.workspace_root.clone();
    let mut slot_map = state.upgrade_fixture.state.lock().await;
    let fixture_state = slot_map.remove(&fixture_id).unwrap_or_default();
    drop(slot_map);
    let result = tokio::task::spawn_blocking(move || {
        dispatch_action(action, body, &workspace_root, fixture_state)
    })
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("fixture task panicked: {err}"),
        )
    })?;
    let (updated_state, value) = result?;
    let mut slot_map = state.upgrade_fixture.state.lock().await;
    slot_map.insert(fixture_id, updated_state);
    Ok(value)
}

fn extract_fixture_id(body: &Value) -> Result<FixtureId, (StatusCode, String)> {
    let raw = body
        .get("fixture_id")
        .and_then(|v| v.as_str())
        .unwrap_or(FixtureId::DEFAULT);
    FixtureId::parse(raw)
}

type ActionResult = Result<(UpgradeFixtureState, Json<Value>), (StatusCode, String)>;

fn dispatch_action(
    action: FixtureAction,
    body: Value,
    workspace_root: &Path,
    mut st: UpgradeFixtureState,
) -> ActionResult {
    let ok = || Json(serde_json::json!({ "ok": true }));
    match action {
        FixtureAction::Reset => {
            action_reset(&mut st)?;
            Ok((st, ok()))
        }
        FixtureAction::WriteFile => {
            action_write_file(body, &mut st)?;
            Ok((st, ok()))
        }
        FixtureAction::RecordBaseline => {
            action_record_baseline(body, &mut st)?;
            Ok((st, ok()))
        }
        FixtureAction::SeedInstall => {
            action_seed_install(body, workspace_root, &mut st)?;
            Ok((st, ok()))
        }
        FixtureAction::MarkLegacyMigrationConcern => {
            action_mark_legacy_migration(body, &mut st)?;
            Ok((st, ok()))
        }
        FixtureAction::CaptureSnapshot => {
            action_capture_snapshot(body, workspace_root, &mut st)?;
            Ok((st, ok()))
        }
        FixtureAction::RunUpgradePreview | FixtureAction::RunUpgradeApply => {
            let confirm = action == FixtureAction::RunUpgradeApply;
            action_run_upgrade(confirm, workspace_root, &mut st)?;
            let val = last_run_json(&st);
            Ok((st, val))
        }
        FixtureAction::AssertNoWrites => {
            action_assert_no_writes(workspace_root, &st)?;
            Ok((st, ok()))
        }
        FixtureAction::AssertMatchesSnapshot => {
            action_assert_matches_snapshot(body, workspace_root, &st)?;
            Ok((st, ok()))
        }
        FixtureAction::AssertPreservesBaseline => {
            action_assert_preserves_baseline(body, &st)?;
            Ok((st, ok()))
        }
        FixtureAction::AssertReplacedFromBaseline => {
            action_assert_replaced_from_baseline(body, &st)?;
            Ok((st, ok()))
        }
        FixtureAction::AssertFileMissing => {
            action_assert_file_missing(body, &st)?;
            Ok((st, ok()))
        }
        FixtureAction::LastRun => {
            let val = last_run_json(&st);
            Ok((st, val))
        }
    }
}

fn last_run_json(st: &UpgradeFixtureState) -> Json<Value> {
    match st.last_run.as_ref() {
        Some(r) => Json(
            serde_json::json!({"ok": true, "stdout": r.stdout, "status": r.status, "success": r.success}),
        ),
        None => Json(serde_json::json!({"ok": true, "stdout": "", "status": 0, "success": false})),
    }
}

// -- Action implementations -----------------------------------------------

fn action_reset(st: &mut UpgradeFixtureState) -> Result<(), (StatusCode, String)> {
    if let Some(path) = st.repository_root.take() {
        let _ = fs::remove_dir_all(path);
    }
    let root = create_fixture_root()?;
    st.repository_root = Some(root);
    st.labeled_snapshots.clear();
    st.file_baselines.clear();
    st.tracked_snapshot_paths.clear();
    st.snapshot_before_last_run = None;
    st.last_run = None;
    Ok(())
}

fn action_write_file(
    body: Value,
    st: &mut UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let p: FileWriteBody = decode_payload(body)?;
    limits::validate_repo_relative_path_len(&p.path)?;
    limits::validate_content_len(p.content.as_bytes())?;
    let root = require_repository_root(st)?.to_path_buf();
    let relative = parse_repository_path(&p.path)?;
    st.tracked_snapshot_paths
        .insert(relative.as_str().to_owned());
    let absolute = root.join(relative.as_str());
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| io_error(&source, parent, "create parent directories"))?;
    }
    fs::write(&absolute, p.content.as_bytes())
        .map_err(|source| io_error(&source, &absolute, "write repository fixture file"))?;
    Ok(())
}

fn action_record_baseline(
    body: Value,
    st: &mut UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let p: FilePathBody = decode_payload(body)?;
    limits::validate_repo_relative_path_len(&p.path)?;
    let root = require_repository_root(st)?.to_path_buf();
    let relative = parse_repository_path(&p.path)?;
    let absolute = root.join(relative.as_str());
    let content = fs::read(&absolute)
        .map_err(|source| io_error(&source, &absolute, "read repository fixture file"))?;
    st.file_baselines
        .insert(relative.as_str().to_owned(), content);
    Ok(())
}

fn action_seed_install(
    body: Value,
    workspace_root: &Path,
    st: &mut UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let p: InstallSeedBody = decode_payload(body)?;
    let label = SnapshotLabel::parse(&p.snapshot_label)?;
    let root = require_repository_root(st)?.to_path_buf();
    let result = apply_install(&root, &p.profile, Some(&p.integrations))
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let _ = result;
    capture_labeled_snapshot(workspace_root, st, label)
}

fn action_mark_legacy_migration(
    body: Value,
    st: &mut UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let p: FilePathBody = decode_payload(body)?;
    limits::validate_repo_relative_path_len(&p.path)?;
    let root = require_repository_root(st)?.to_path_buf();
    let relative = parse_repository_path(&p.path)?;
    st.tracked_snapshot_paths
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
    Ok(())
}

fn action_capture_snapshot(
    body: Value,
    workspace_root: &Path,
    st: &mut UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let p: SnapshotBody = decode_payload(body)?;
    let label = SnapshotLabel::parse(&p.label)?;
    capture_labeled_snapshot(workspace_root, st, label)
}

fn action_run_upgrade(
    confirm: bool,
    workspace_root: &Path,
    st: &mut UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let root = require_repository_root(st)?.to_path_buf();
    let snapshot = capture_scoped_snapshot(&root, workspace_root, &st.tracked_snapshot_paths)?;
    st.snapshot_before_last_run = Some(snapshot);
    let result = run_upgrade_witness(&root, confirm);
    match result {
        Ok(run) => {
            st.last_run = Some(CommandResult {
                stdout: run.stdout(),
                status: 0,
                success: true,
            });
        }
        Err(err) => {
            st.last_run = Some(CommandResult {
                stdout: err.to_string(),
                status: 1,
                success: false,
            });
        }
    }
    Ok(())
}

fn action_assert_no_writes(
    workspace_root: &Path,
    st: &UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let before = st.snapshot_before_last_run.as_ref().ok_or((
        StatusCode::BAD_REQUEST,
        "no snapshot captured before last run".to_owned(),
    ))?;
    let root = require_repository_root(st)?;
    let current = capture_scoped_snapshot(root, workspace_root, &st.tracked_snapshot_paths)?;
    if current != *before {
        return Err((
            StatusCode::CONFLICT,
            "repository was written to despite no-confirm preview".to_owned(),
        ));
    }
    Ok(())
}

fn action_assert_matches_snapshot(
    body: Value,
    workspace_root: &Path,
    st: &UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let p: SnapshotBody = decode_payload(body)?;
    let label = SnapshotLabel::parse(&p.label)?;
    let expected = st.labeled_snapshots.get(&label).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            format!("no snapshot labeled '{label}'"),
        )
    })?;
    let root = require_repository_root(st)?;
    let current = capture_scoped_snapshot(root, workspace_root, &st.tracked_snapshot_paths)?;
    if current != *expected {
        return Err((
            StatusCode::CONFLICT,
            format!("current repository state does not match snapshot '{label}'"),
        ));
    }
    Ok(())
}

fn read_baseline_for_assert(
    body: Value,
    st: &UpgradeFixtureState,
) -> Result<(PathBuf, RepoRelativePath, Vec<u8>), (StatusCode, String)> {
    let p: FilePathBody = decode_payload(body)?;
    limits::validate_repo_relative_path_len(&p.path)?;
    let root = require_repository_root(st)?.to_path_buf();
    let relative = parse_repository_path(&p.path)?;
    let baseline = st.file_baselines.get(relative.as_str()).cloned().ok_or((
        StatusCode::BAD_REQUEST,
        format!("missing baseline for '{}'", relative.as_str()),
    ))?;
    Ok((root, relative, baseline))
}

fn action_assert_preserves_baseline(
    body: Value,
    st: &UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let (root, relative, baseline) = read_baseline_for_assert(body, st)?;
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
    Ok(())
}

fn action_assert_replaced_from_baseline(
    body: Value,
    st: &UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let (root, relative, baseline) = read_baseline_for_assert(body, st)?;
    let absolute = root.join(relative.as_str());
    let current = fs::read(&absolute)
        .map_err(|source| io_error(&source, &absolute, "read repository fixture file"))?;
    if current == baseline {
        return Err((
            StatusCode::CONFLICT,
            format!("expected '{}' to differ from baseline", relative.as_str()),
        ));
    }
    Ok(())
}

fn action_assert_file_missing(
    body: Value,
    st: &UpgradeFixtureState,
) -> Result<(), (StatusCode, String)> {
    let p: FilePathBody = decode_payload(body)?;
    limits::validate_repo_relative_path_len(&p.path)?;
    let root = require_repository_root(st)?.to_path_buf();
    let relative = parse_repository_path(&p.path)?;
    let absolute = root.join(relative.as_str());
    match fs::metadata(&absolute) {
        Ok(_) => Err((
            StatusCode::CONFLICT,
            format!("expected '{}' to be absent", relative.as_str()),
        )),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error(
            &source,
            &absolute,
            "inspect repository fixture path",
        )),
    }
}

// -- Internal helpers ------------------------------------------------------

fn create_fixture_root() -> Result<PathBuf, (StatusCode, String)> {
    let now_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let sequence = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "tanren-web-bdd-install-{}-{sequence}-{now_nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&path).map_err(|source| io_error(&source, &path, "create fixture root"))?;
    Ok(path)
}

fn capture_labeled_snapshot(
    workspace_root: &Path,
    st: &mut UpgradeFixtureState,
    label: SnapshotLabel,
) -> Result<(), (StatusCode, String)> {
    let root = require_repository_root(st)?;
    let snapshot = capture_scoped_snapshot(root, workspace_root, &st.tracked_snapshot_paths)?;
    st.labeled_snapshots.insert(label, snapshot);
    Ok(())
}

fn require_repository_root(st: &UpgradeFixtureState) -> Result<&Path, (StatusCode, String)> {
    st.repository_root.as_deref().ok_or((
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
