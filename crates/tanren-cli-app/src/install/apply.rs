use std::collections::HashSet;
use std::fmt::Write;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::install::assets::EmbeddedFile;
use crate::install::manifest::{self, FileEntry, Manifest, PriorManifest};
use crate::install::render::RenderedIntegration;

pub(crate) struct InstallOutcome {
    pub(crate) created: Vec<String>,
    pub(crate) updated: Vec<String>,
    pub(crate) removed: Vec<String>,
    pub(crate) restored: Vec<String>,
    pub(crate) preserved: Vec<String>,
}

pub(crate) struct InstallResult {
    pub(crate) manifest: Manifest,
    pub(crate) outcome: InstallOutcome,
}

pub(crate) fn apply(
    repo: &Path,
    profile: &str,
    commands: &[EmbeddedFile],
    integrations: &[RenderedIntegration],
    standards: &[EmbeddedFile],
) -> Result<InstallResult> {
    let tanren_dir = repo.join(".tanren");
    fs::create_dir_all(&tanren_dir).with_context(|| format!("create {}", tanren_dir.display()))?;

    let prior = manifest::read_prior_manifest(repo);

    let gen_out = write_generated(repo, commands, integrations)?;
    let removed = remove_stale(repo, prior.as_ref(), &gen_out.new_paths)?;
    let std_out = write_standards(repo, standards, prior.as_ref())?;

    let new_manifest = Manifest {
        profile: profile.to_owned(),
        generated: gen_out.generated,
        standards: std_out.entries,
    };

    let mut all_updated = gen_out.updated;
    all_updated.extend(std_out.updated);

    manifest::write_manifest(repo, &new_manifest)?;

    Ok(InstallResult {
        manifest: new_manifest,
        outcome: InstallOutcome {
            created: gen_out.created,
            updated: all_updated,
            removed,
            restored: std_out.restored,
            preserved: std_out.preserved,
        },
    })
}

struct GeneratedOutput {
    generated: Vec<FileEntry>,
    new_paths: HashSet<String>,
    created: Vec<String>,
    updated: Vec<String>,
}

fn write_generated(
    repo: &Path,
    commands: &[EmbeddedFile],
    integrations: &[RenderedIntegration],
) -> Result<GeneratedOutput> {
    let mut generated: Vec<FileEntry> = Vec::new();
    let mut new_paths: HashSet<String> = HashSet::new();
    let mut created: Vec<String> = Vec::new();
    let mut updated: Vec<String> = Vec::new();

    for cmd in commands {
        new_paths.insert(cmd.relative_path.clone());
        let existed = repo.join(&cmd.relative_path).exists();
        write_file(repo, &cmd.relative_path, cmd.content)?;
        if existed {
            updated.push(cmd.relative_path.clone());
        } else {
            created.push(cmd.relative_path.clone());
        }
        generated.push(FileEntry {
            path: cmd.relative_path.clone(),
            hash: content_hash(cmd.content),
        });
    }

    for integration in integrations {
        new_paths.insert(integration.relative_path.clone());
        let existed = repo.join(&integration.relative_path).exists();
        write_file(
            repo,
            &integration.relative_path,
            integration.content.as_bytes(),
        )?;
        if existed {
            updated.push(integration.relative_path.clone());
        } else {
            created.push(integration.relative_path.clone());
        }
        generated.push(FileEntry {
            path: integration.relative_path.clone(),
            hash: content_hash(integration.content.as_bytes()),
        });
    }

    Ok(GeneratedOutput {
        generated,
        new_paths,
        created,
        updated,
    })
}

fn remove_stale(
    repo: &Path,
    prior: Option<&PriorManifest>,
    new_paths: &HashSet<String>,
) -> Result<Vec<String>> {
    let Some(prior) = prior else {
        return Ok(Vec::new());
    };
    let mut removed: Vec<String> = Vec::new();
    for entry in &prior.generated {
        if !new_paths.contains(&entry.path) {
            let full_path = repo.join(&entry.path);
            if full_path.exists() {
                fs::remove_file(&full_path)
                    .with_context(|| format!("remove stale {}", full_path.display()))?;
            }
            removed.push(entry.path.clone());
        }
    }
    Ok(removed)
}

struct StandardsOutput {
    entries: Vec<FileEntry>,
    updated: Vec<String>,
    restored: Vec<String>,
    preserved: Vec<String>,
}

fn write_standards(
    repo: &Path,
    standards: &[EmbeddedFile],
    prior: Option<&PriorManifest>,
) -> Result<StandardsOutput> {
    let mut entries: Vec<FileEntry> = Vec::new();
    let mut updated: Vec<String> = Vec::new();
    let mut restored: Vec<String> = Vec::new();
    let mut preserved: Vec<String> = Vec::new();

    for standard in standards {
        let full_path = repo.join(&standard.relative_path);
        let prior_entry = prior.as_ref().and_then(|p| {
            p.standards
                .iter()
                .find(|e| e.path == standard.relative_path)
        });
        let on_disk = full_path.exists();

        if !on_disk {
            write_file(repo, &standard.relative_path, standard.content)?;
            let hash = content_hash(standard.content);
            if prior_entry.is_some() {
                restored.push(standard.relative_path.clone());
            }
            entries.push(FileEntry {
                path: standard.relative_path.clone(),
                hash,
            });
        } else if let Some(prior_file) = prior_entry {
            let disk_content =
                fs::read(&full_path).with_context(|| format!("read {}", full_path.display()))?;
            let disk_hash = content_hash(&disk_content);
            if disk_hash == prior_file.hash {
                write_file(repo, &standard.relative_path, standard.content)?;
                let hash = content_hash(standard.content);
                updated.push(standard.relative_path.clone());
                entries.push(FileEntry {
                    path: standard.relative_path.clone(),
                    hash,
                });
            } else {
                preserved.push(standard.relative_path.clone());
                entries.push(FileEntry {
                    path: standard.relative_path.clone(),
                    hash: disk_hash,
                });
            }
        } else {
            write_file(repo, &standard.relative_path, standard.content)?;
            let hash = content_hash(standard.content);
            updated.push(standard.relative_path.clone());
            entries.push(FileEntry {
                path: standard.relative_path.clone(),
                hash,
            });
        }
    }

    Ok(StandardsOutput {
        entries,
        updated,
        restored,
        preserved,
    })
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
