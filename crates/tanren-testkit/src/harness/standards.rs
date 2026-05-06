use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use tanren_app_services::standards::projection::{
    DriftStatus, ProjectionBounds, ProjectionEntry, ProjectionInspector, ProjectionManifest,
};
use tanren_app_services::standards::{
    StandardsReadModel, clear_standards_root, configure_standards_root,
};
use tanren_contract::StandardSchema;

use super::{HarnessError, HarnessResult};

pub struct StandardsCliRunner {
    binary: PathBuf,
}

impl std::fmt::Debug for StandardsCliRunner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandardsCliRunner")
            .field("binary", &self.binary)
            .finish_non_exhaustive()
    }
}

impl StandardsCliRunner {
    pub fn new() -> HarnessResult<Self> {
        let binary = super::cli::locate_workspace_binary("tanren-cli")?;
        Ok(Self { binary })
    }

    pub async fn inspect(&self, project_dir: &Path) -> StandardsInspectResult {
        let output = tokio::process::Command::new(&self.binary)
            .args([
                "standards",
                "inspect",
                "--project-dir",
                &project_dir.display().to_string(),
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await;
        match output {
            Ok(out) => StandardsInspectResult {
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
                success: out.status.success(),
            },
            Err(e) => StandardsInspectResult {
                stdout: String::new(),
                stderr: format!("failed to spawn tanren-cli: {e}"),
                success: false,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct StandardsInspectResult {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

pub fn create_temp_project_dir(prefix: &str) -> HarnessResult<PathBuf> {
    let base = std::env::temp_dir();
    let dir = base.join(format!("tanren-bdd-{}-{}", prefix, uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir)
        .map_err(|e| HarnessError::Transport(format!("create temp dir: {e}")))?;
    Ok(dir)
}

pub fn write_project_config(project_dir: &Path, standards_root: &str) -> HarnessResult<()> {
    let config = format!("schema: tanren.project.v0\nstandards:\n  root: {standards_root}\n");
    std::fs::write(project_dir.join("tanren.yml"), config)
        .map_err(|e| HarnessError::Transport(format!("write tanren.yml: {e}")))?;
    Ok(())
}

pub fn write_valid_standard(standards_dir: &Path, name: String) -> HarnessResult<()> {
    std::fs::create_dir_all(standards_dir)
        .map_err(|e| HarnessError::Transport(format!("create standards dir: {e}")))?;
    let content = format!(
        "---\nkind: standard\nname: {name}\ncategory: quality\nimportance: high\n---\n\n# {name}\n"
    );
    std::fs::write(standards_dir.join(name + ".md"), content)
        .map_err(|e| HarnessError::Transport(format!("write standard file: {e}")))?;
    Ok(())
}

pub fn write_malformed_standard(standards_dir: &Path, name: String) -> HarnessResult<()> {
    std::fs::create_dir_all(standards_dir)
        .map_err(|e| HarnessError::Transport(format!("create standards dir: {e}")))?;
    let content =
        format!("---\nkind: standard\nname: {name}\ncategory: [invalid\n---\n\n# {name}\n");
    std::fs::write(standards_dir.join(name + ".md"), content)
        .map_err(|e| HarnessError::Transport(format!("write malformed standard file: {e}")))?;
    Ok(())
}

pub fn seed_standards_read_model(root: PathBuf) -> StandardsReadModel {
    let event = configure_standards_root(root, StandardSchema::current(), Utc::now());
    let mut rm = StandardsReadModel::default();
    rm.apply_event(&event);
    rm
}

pub fn build_standards_configured_event(root: PathBuf, at: DateTime<Utc>) -> serde_json::Value {
    configure_standards_root(root, StandardSchema::current(), at)
}

pub fn build_standards_cleared_event(at: DateTime<Utc>) -> serde_json::Value {
    clear_standards_root(at)
}

pub fn replay_standards_events(events: &[serde_json::Value]) -> StandardsReadModel {
    let mut rm = StandardsReadModel::default();
    rm.apply_events(events);
    rm
}

fn compute_digest(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    let mut hex = String::with_capacity(hash.len() * 2);
    for byte in &hash {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

pub fn make_projection_entry(relative_path: PathBuf, content: &[u8]) -> ProjectionEntry {
    ProjectionEntry {
        content_digest: compute_digest(content),
        size_bytes: content.len() as u64,
        relative_path,
    }
}

pub fn make_projection_manifest(entries: Vec<ProjectionEntry>) -> ProjectionManifest {
    ProjectionManifest { entries }
}

pub fn make_inspector_with_bounds(bounds: ProjectionBounds) -> ProjectionInspector {
    ProjectionInspector::new(bounds)
}

pub fn make_default_inspector() -> ProjectionInspector {
    ProjectionInspector::with_default_bounds()
}

pub fn write_projection_entry(
    standards_dir: &Path,
    entry: &ProjectionEntry,
    content: &[u8],
) -> HarnessResult<()> {
    let file_path = standards_dir.join(&entry.relative_path);
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| HarnessError::Transport(format!("create dir: {e}")))?;
    }
    std::fs::write(&file_path, content)
        .map_err(|e| HarnessError::Transport(format!("write projection entry: {e}")))?;
    Ok(())
}

pub fn run_projection_inspection(
    root: &Path,
    manifest: &ProjectionManifest,
) -> Result<Vec<(PathBuf, DriftStatus)>, HarnessError> {
    let inspector = ProjectionInspector::with_default_bounds();
    let report = inspector
        .inspect(root, manifest)
        .map_err(|e| HarnessError::Transport(format!("projection inspection failed: {e}")))?;
    Ok(report
        .items
        .into_iter()
        .map(|item| (item.relative_path, item.status))
        .collect())
}
