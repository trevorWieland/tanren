use std::fmt::Write;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::install::assets::EmbeddedFile;
use crate::install::render::RenderedIntegration;

pub(crate) struct Manifest {
    pub(crate) profile: String,
    pub(crate) generated: Vec<FileEntry>,
    pub(crate) standards: Vec<FileEntry>,
}

pub(crate) struct FileEntry {
    pub(crate) path: String,
    pub(crate) hash: String,
}

pub(crate) fn apply(
    repo: &Path,
    profile: &str,
    commands: &[EmbeddedFile],
    integrations: &[RenderedIntegration],
    standards: &[EmbeddedFile],
) -> Result<Manifest> {
    let tanren_dir = repo.join(".tanren");
    fs::create_dir_all(&tanren_dir).with_context(|| format!("create {}", tanren_dir.display()))?;

    let mut generated = Vec::new();

    for cmd in commands {
        write_file(repo, &cmd.relative_path, cmd.content)?;
        let hash = content_hash(cmd.content);
        generated.push(FileEntry {
            path: cmd.relative_path.clone(),
            hash,
        });
    }

    for integration in integrations {
        write_file(
            repo,
            &integration.relative_path,
            integration.content.as_bytes(),
        )?;
        let hash = content_hash(integration.content.as_bytes());
        generated.push(FileEntry {
            path: integration.relative_path.clone(),
            hash,
        });
    }

    let mut standard_entries = Vec::new();

    for standard in standards {
        write_file(repo, &standard.relative_path, standard.content)?;
        let hash = content_hash(standard.content);
        standard_entries.push(FileEntry {
            path: standard.relative_path.clone(),
            hash,
        });
    }

    let manifest = Manifest {
        profile: profile.to_owned(),
        generated,
        standards: standard_entries,
    };

    write_manifest(repo, &manifest)?;

    Ok(manifest)
}

fn write_file(repo: &Path, relative_path: &str, content: &[u8]) -> Result<()> {
    let full_path = repo.join(relative_path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(&full_path, content).with_context(|| format!("write {}", full_path.display()))
}

fn content_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    let result = hasher.finalize();
    let mut hex = String::with_capacity(result.len() * 2);
    for b in &result {
        let _ = write!(hex, "{b:02x}");
    }
    format!("sha256:{hex}")
}

fn write_manifest(repo: &Path, manifest: &Manifest) -> Result<()> {
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
