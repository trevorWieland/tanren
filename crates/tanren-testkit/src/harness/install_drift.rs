use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use tanren_app_services::install::PROJECTION_MANIFEST;
use tanren_contract::InstallDriftAssetKind;
use tokio::process::Command;
use uuid::Uuid;

use super::cli::locate_workspace_binary;

#[derive(Debug, Clone, Deserialize)]
pub struct DriftReport {
    pub has_drift: bool,
    pub entries: Vec<DriftEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DriftEntry {
    pub relative_path: String,
    pub state: String,
}

pub struct InstallDriftFixture {
    repo_dir: PathBuf,
    binary: PathBuf,
}

impl std::fmt::Debug for InstallDriftFixture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstallDriftFixture")
            .field("repo_dir", &self.repo_dir)
            .finish_non_exhaustive()
    }
}

impl InstallDriftFixture {
    pub fn new() -> Result<Self> {
        let binary = locate_workspace_binary("tanren-cli")?;
        let unique_dir = format!("tanren-drift-bdd-{}", Uuid::new_v4().simple());
        let repo_dir = std::env::temp_dir().join(unique_dir);
        fs::create_dir_all(&repo_dir)
            .with_context(|| format!("create fixture dir {}", repo_dir.display()))?;

        let fixture = Self { repo_dir, binary };
        fixture.populate_assets()?;
        Ok(fixture)
    }

    pub fn repo_dir(&self) -> &Path {
        &self.repo_dir
    }

    pub fn modify_generated_asset(&self) -> Result<()> {
        let entry = first_generated();
        let full_path = self.repo_dir.join(entry.rel_path);
        ensure!(
            full_path.exists(),
            "generated asset {} not found in fixture",
            entry.rel_path
        );
        let existing = fs::read_to_string(&full_path)
            .with_context(|| format!("read {}", full_path.display()))?;
        fs::write(&full_path, format!("{existing}\n// drift injected"))
            .with_context(|| format!("modify {}", full_path.display()))?;
        Ok(())
    }

    pub fn delete_generated_asset(&self) -> Result<()> {
        let entry = first_generated();
        let full_path = self.repo_dir.join(entry.rel_path);
        ensure!(
            full_path.exists(),
            "generated asset {} not found in fixture",
            entry.rel_path
        );
        fs::remove_file(&full_path).with_context(|| format!("delete {}", full_path.display()))?;
        Ok(())
    }

    pub fn delete_preserved_standard(&self) -> Result<()> {
        let entry = first_preserved();
        let full_path = self.repo_dir.join(entry.rel_path);
        ensure!(
            full_path.exists(),
            "preserved standard {} not found in fixture",
            entry.rel_path
        );
        fs::remove_file(&full_path).with_context(|| format!("delete {}", full_path.display()))?;
        Ok(())
    }

    pub fn edit_preserved_standard(&self) -> Result<()> {
        let entry = first_preserved();
        let full_path = self.repo_dir.join(entry.rel_path);
        ensure!(
            full_path.exists(),
            "preserved standard {} not found in fixture",
            entry.rel_path
        );
        let existing = fs::read_to_string(&full_path)
            .with_context(|| format!("read {}", full_path.display()))?;
        fs::write(&full_path, format!("{existing}\nUser customization."))
            .with_context(|| format!("edit {}", full_path.display()))?;
        Ok(())
    }

    pub async fn run_drift_check(&self) -> Result<DriftReport> {
        let output = Command::new(&self.binary)
            .args([
                "install",
                "drift",
                "--repo",
                &self.repo_dir.to_string_lossy(),
                "--format",
                "json",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .with_context(|| "spawn tanren-cli install drift")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let report: DriftReport = serde_json::from_str(stdout.trim())
            .with_context(|| format!("parse drift report from stdout: {stdout}"))?;
        Ok(report)
    }

    pub fn snapshot_files(&self) -> Result<HashMap<String, Vec<u8>>> {
        let mut snapshots = HashMap::new();
        for entry in PROJECTION_MANIFEST {
            let full_path = self.repo_dir.join(entry.rel_path);
            if full_path.exists() {
                let bytes = fs::read(&full_path)
                    .with_context(|| format!("snapshot {}", full_path.display()))?;
                snapshots.insert(entry.rel_path.to_string(), bytes);
            }
        }
        Ok(snapshots)
    }

    fn populate_assets(&self) -> Result<()> {
        for entry in PROJECTION_MANIFEST {
            let full_path = self.repo_dir.join(entry.rel_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create {}", parent.display()))?;
            }
            match entry.kind {
                InstallDriftAssetKind::Generated => {
                    let content = entry
                        .expected_content
                        .expect("generated entries carry expected content");
                    fs::write(&full_path, content)
                        .with_context(|| format!("write {}", full_path.display()))?;
                }
                InstallDriftAssetKind::PreservedStandard => {
                    fs::write(&full_path, "# Tech Stack\nPlaceholder standard.\n")
                        .with_context(|| format!("write {}", full_path.display()))?;
                }
            }
        }
        Ok(())
    }
}

impl Drop for InstallDriftFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.repo_dir);
    }
}

fn first_generated() -> &'static tanren_app_services::install::ProjectionEntry {
    PROJECTION_MANIFEST
        .iter()
        .find(|e| e.kind == InstallDriftAssetKind::Generated)
        .expect("manifest must contain at least one generated asset")
}

fn first_preserved() -> &'static tanren_app_services::install::ProjectionEntry {
    PROJECTION_MANIFEST
        .iter()
        .find(|e| e.kind == InstallDriftAssetKind::PreservedStandard)
        .expect("manifest must contain at least one preserved standard")
}
