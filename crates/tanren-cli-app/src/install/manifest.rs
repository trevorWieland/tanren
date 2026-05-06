use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

pub(crate) struct Manifest {
    pub(crate) profile: String,
    pub(crate) generated: Vec<FileEntry>,
    pub(crate) standards: Vec<FileEntry>,
}

pub(crate) struct FileEntry {
    pub(crate) path: String,
    pub(crate) hash: String,
}

pub(crate) struct PriorManifest {
    pub(crate) generated: Vec<FileEntry>,
    pub(crate) standards: Vec<FileEntry>,
}

pub(crate) fn read_prior_manifest(repo: &Path) -> Option<PriorManifest> {
    let manifest_path = repo.join(".tanren/install-manifest.json");
    if !manifest_path.exists() {
        return None;
    }
    let content = fs::read_to_string(&manifest_path).ok()?;
    parse_manifest(&content)
}

fn parse_manifest(content: &str) -> Option<PriorManifest> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;

    let generated = json
        .get("generated_files")?
        .as_array()?
        .iter()
        .filter_map(|v| {
            Some(FileEntry {
                path: v.get("path")?.as_str()?.to_owned(),
                hash: v.get("hash")?.as_str()?.to_owned(),
            })
        })
        .collect();

    let standards = json
        .get("standard_files")?
        .as_array()?
        .iter()
        .filter_map(|v| {
            Some(FileEntry {
                path: v.get("path")?.as_str()?.to_owned(),
                hash: v.get("hash")?.as_str()?.to_owned(),
            })
        })
        .collect();

    Some(PriorManifest {
        generated,
        standards,
    })
}

pub(crate) fn write_manifest(repo: &Path, manifest: &Manifest) -> Result<()> {
    let generated: Vec<serde_json::Value> = manifest
        .generated
        .iter()
        .map(|e| {
            serde_json::json!({
                "path": e.path,
                "hash": e.hash,
            })
        })
        .collect();

    let standards: Vec<serde_json::Value> = manifest
        .standards
        .iter()
        .map(|e| {
            serde_json::json!({
                "path": e.path,
                "hash": e.hash,
            })
        })
        .collect();

    let json = serde_json::json!({
        "profile": manifest.profile,
        "generated_files": generated,
        "standard_files": standards,
    });

    let manifest_path = repo.join(".tanren").join("install-manifest.json");
    let formatted = serde_json::to_string_pretty(&json).context("format manifest JSON")?;
    fs::write(&manifest_path, formatted)
        .with_context(|| format!("write {}", manifest_path.display()))
}
