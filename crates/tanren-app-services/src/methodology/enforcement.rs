//! Three-layer enforcement of orchestrator-owned artifacts.
//!
//! Per Lane 0.5 non-negotiable #4, agents never write orchestrator-
//! owned artifacts (`plan.md`, `progress.json`, generated indexes).
//! Enforcement has three layers:
//!
//! 1. **Prompt banner**: `{{READONLY_ARTIFACT_BANNER}}` template
//!    variable rendered into every agent command.
//! 2. **Filesystem chmod**: pre-session `chmod 0444` on each protected
//!    path, restored to original mode on session exit.
//! 3. **Postflight diff + revert**: on session exit, compare the
//!    on-disk bytes to the pre-session snapshot; any mismatch reverts
//!    from the snapshot and emits
//!    [`UnauthorizedArtifactEdit`](tanren_domain::methodology::events::UnauthorizedArtifactEdit).
//!
//! This module owns the library surface used by both the dedicated
//! `tanren session enter|exit` CLI subcommands and any future
//! orchestrator that wants to drive the same guarantees programmatically.

use std::path::{Path, PathBuf};

use super::errors::{MethodologyError, MethodologyResult};

/// Pre-session snapshot of one protected file.
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub existed_before: bool,
    pub original_mode: Option<u32>,
    pub pre_bytes: Vec<u8>,
}

/// Result of one postflight verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnauthorizedEdit {
    pub path: PathBuf,
    pub diff_preview: String,
}

/// Enforcement guard — holds the pre-session snapshot list and exposes
/// `verify_and_exit`. Construction snapshots the files and chmods them
/// read-only; drop-without-exit leaves files chmodded (callers should
/// treat the type as linear).
#[derive(Debug)]
pub struct EnforcementGuard {
    snapshots: Vec<FileSnapshot>,
}

impl EnforcementGuard {
    /// Snapshot the given paths and `chmod 0444` each.
    ///
    /// # Errors
    /// Returns [`MethodologyError::Io`] on any read / chmod failure.
    pub fn enter(paths: &[PathBuf]) -> MethodologyResult<Self> {
        let mut snapshots = Vec::with_capacity(paths.len());
        for path in paths {
            if path.exists() {
                let bytes = std::fs::read(path).map_err(|source| MethodologyError::Io {
                    path: path.clone(),
                    source,
                })?;
                let mode = file_mode(path)?;
                snapshots.push(FileSnapshot {
                    path: path.clone(),
                    existed_before: true,
                    original_mode: Some(mode),
                    pre_bytes: bytes,
                });
                set_mode(path, 0o444)?;
            } else {
                snapshots.push(FileSnapshot {
                    path: path.clone(),
                    existed_before: false,
                    original_mode: None,
                    pre_bytes: Vec::new(),
                });
            }
        }
        Ok(Self { snapshots })
    }

    /// Verify each protected file hasn't been modified; revert any
    /// that have; restore original modes; emit unauthorized-edit
    /// descriptions for the caller to convert into events.
    ///
    /// # Errors
    /// Returns [`MethodologyError::Io`] on any read / write / chmod
    /// failure. Files that verify clean are restored to their
    /// original mode.
    pub fn verify_and_exit(self) -> MethodologyResult<Vec<UnauthorizedEdit>> {
        let mut edits = Vec::new();
        let mut watched_dirs = std::collections::BTreeSet::new();
        let mut known_paths = std::collections::BTreeSet::new();
        for snap in &self.snapshots {
            known_paths.insert(snap.path.clone());
            if let Some(parent) = snap.path.parent() {
                watched_dirs.insert(parent.to_path_buf());
            }
        }

        for snap in self.snapshots {
            if !snap.existed_before {
                if snap.path.exists() {
                    if snap.path.is_dir() {
                        std::fs::remove_dir_all(&snap.path).map_err(|source| {
                            MethodologyError::Io {
                                path: snap.path.clone(),
                                source,
                            }
                        })?;
                    } else {
                        std::fs::remove_file(&snap.path).map_err(|source| {
                            MethodologyError::Io {
                                path: snap.path.clone(),
                                source,
                            }
                        })?;
                    }
                    edits.push(UnauthorizedEdit {
                        path: snap.path.clone(),
                        diff_preview: "file was created during session and removed".into(),
                    });
                }
                continue;
            }

            let current = match std::fs::read(&snap.path) {
                Ok(bytes) => Some(bytes),
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => None,
                Err(source) => {
                    return Err(MethodologyError::Io {
                        path: snap.path.clone(),
                        source,
                    });
                }
            };
            let changed = current.as_deref() != Some(snap.pre_bytes.as_slice());
            if changed {
                if snap.path.exists() {
                    set_mode(&snap.path, 0o644)?;
                } else if let Some(parent) = snap.path.parent() {
                    std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
                        path: parent.to_path_buf(),
                        source,
                    })?;
                }
                std::fs::write(&snap.path, &snap.pre_bytes).map_err(|source| {
                    MethodologyError::Io {
                        path: snap.path.clone(),
                        source,
                    }
                })?;
                let diff_preview = match current {
                    Some(bytes) => summarize_diff(&snap.pre_bytes, &bytes),
                    None => "file was deleted during session and restored".into(),
                };
                edits.push(UnauthorizedEdit {
                    path: snap.path.clone(),
                    diff_preview,
                });
            }
            if let Some(mode) = snap.original_mode {
                set_mode(&snap.path, mode)?;
            }
        }

        for root in watched_dirs {
            let Ok(entries) = std::fs::read_dir(&root) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() || known_paths.contains(&path) {
                    continue;
                }
                if !is_protected_generated_index(&path) {
                    continue;
                }
                std::fs::remove_file(&path).map_err(|source| MethodologyError::Io {
                    path: path.clone(),
                    source,
                })?;
                edits.push(UnauthorizedEdit {
                    path,
                    diff_preview: "file was created during session and removed".into(),
                });
            }
        }
        Ok(edits)
    }
}

/// Read the file's current mode bits.
fn file_mode(path: &Path) -> MethodologyResult<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(path)
            .map(|m| m.mode())
            .map_err(|source| MethodologyError::Io {
                path: path.to_path_buf(),
                source,
            })
    }
    #[cfg(not(unix))]
    {
        // Windows: chmod semantics don't apply. Return a sentinel so
        // `set_mode` becomes a no-op. Correctness relies on the
        // prompt-banner + postflight-diff layers instead.
        let _ = path;
        Ok(0)
    }
}

/// Set the file's mode bits.
fn set_mode(path: &Path, mode: u32) -> MethodologyResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(mode);
        std::fs::set_permissions(path, perms).map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
    #[cfg(not(unix))]
    {
        // Windows: no-op. See `file_mode` above.
        let _ = (path, mode);
        Ok(())
    }
}

/// Short textual summary of a byte-level diff for emission in
/// `UnauthorizedArtifactEdit`'s `diff_preview`. Kept to the first
/// differing line pair to avoid blowing up event payloads.
fn summarize_diff(old: &[u8], new: &[u8]) -> String {
    let old_str = String::from_utf8_lossy(old);
    let new_str = String::from_utf8_lossy(new);
    for (i, (a, b)) in old_str.lines().zip(new_str.lines()).enumerate() {
        if a != b {
            return format!("line {}: `{a}` → `{b}`", i + 1);
        }
    }
    let extra = new_str
        .lines()
        .count()
        .saturating_sub(old_str.lines().count());
    if extra > 0 {
        format!("{extra} trailing line(s) added")
    } else {
        format!(
            "byte-level difference; old {}b, new {}b",
            old.len(),
            new.len()
        )
    }
}

fn is_protected_generated_index(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
        return false;
    };
    if !name.contains("index") {
        return false;
    }
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_diff_on_simple_line_change() {
        let summary = summarize_diff(b"a\nb\nc\n", b"a\nX\nc\n");
        assert!(summary.contains("line 2"));
        assert!(summary.contains('X'));
    }

    #[test]
    fn summarize_diff_on_trailing_append() {
        let summary = summarize_diff(b"a\n", b"a\nb\n");
        assert!(summary.contains("trailing line"));
    }
}
