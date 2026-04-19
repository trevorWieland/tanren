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

/// Per-path protection policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProtectionMode {
    ReadOnly,
    AppendOnly,
}

/// Protected file entry used on session enter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedPath {
    pub path: PathBuf,
    pub mode: ProtectionMode,
}

/// Pre-session snapshot of one protected file.
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub mode: ProtectionMode,
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
/// `verify_and_exit`. Construction snapshots the files and applies the
/// policy-specific protections. Drop performs best-effort permission
/// restoration for `ReadOnly` paths when callers fail to finalize.
#[derive(Debug)]
pub struct EnforcementGuard {
    snapshots: Option<Vec<FileSnapshot>>,
}

impl EnforcementGuard {
    /// Snapshot the given paths and `chmod 0444` each.
    ///
    /// # Errors
    /// Returns [`MethodologyError::Io`] on any read / chmod failure.
    pub fn enter(paths: &[ProtectedPath]) -> MethodologyResult<Self> {
        let mut snapshots = Vec::with_capacity(paths.len());
        for protected in paths {
            let path = &protected.path;
            if path.exists() {
                let bytes = std::fs::read(path).map_err(|source| MethodologyError::Io {
                    path: path.clone(),
                    source,
                })?;
                let mode = file_mode(path)?;
                snapshots.push(FileSnapshot {
                    path: path.clone(),
                    mode: protected.mode,
                    existed_before: true,
                    original_mode: Some(mode),
                    pre_bytes: bytes,
                });
                if matches!(protected.mode, ProtectionMode::ReadOnly) {
                    set_mode(path, 0o444)?;
                }
            } else {
                snapshots.push(FileSnapshot {
                    path: path.clone(),
                    mode: protected.mode,
                    existed_before: false,
                    original_mode: None,
                    pre_bytes: Vec::new(),
                });
            }
        }
        Ok(Self {
            snapshots: Some(snapshots),
        })
    }

    /// Verify each protected file hasn't been modified; revert any
    /// that have; restore original modes; emit unauthorized-edit
    /// descriptions for the caller to convert into events.
    ///
    /// # Errors
    /// Returns [`MethodologyError::Io`] on any read / write / chmod
    /// failure. Files that verify clean are restored to their
    /// original mode.
    pub fn verify_and_exit(mut self) -> MethodologyResult<Vec<UnauthorizedEdit>> {
        let snapshots = self.snapshots.take().unwrap_or_default();
        let mut edits = Vec::new();
        let mut watched_dirs = std::collections::BTreeSet::new();
        let mut known_paths = std::collections::BTreeSet::new();
        for snap in &snapshots {
            known_paths.insert(snap.path.clone());
            if let Some(parent) = snap.path.parent() {
                watched_dirs.insert(parent.to_path_buf());
            }
        }

        for snap in &snapshots {
            Self::verify_snapshot(snap, &mut edits)?;
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

    fn verify_snapshot(
        snap: &FileSnapshot,
        edits: &mut Vec<UnauthorizedEdit>,
    ) -> MethodologyResult<()> {
        if !snap.existed_before {
            if snap.path.exists() {
                if matches!(snap.mode, ProtectionMode::AppendOnly) {
                    return Ok(());
                }
                remove_path(&snap.path)?;
                edits.push(UnauthorizedEdit {
                    path: snap.path.clone(),
                    diff_preview: "file was created during session and removed".into(),
                });
            }
            return Ok(());
        }

        let current = read_optional_bytes(&snap.path)?;
        let changed = current.as_deref() != Some(snap.pre_bytes.as_slice());
        if changed {
            if matches!(snap.mode, ProtectionMode::AppendOnly)
                && let Some(bytes) = current.as_deref()
                && bytes.starts_with(&snap.pre_bytes)
            {
                return Ok(());
            }
            restore_snapshot_contents(snap)?;
            let diff_preview = match current {
                Some(bytes) => summarize_diff(&snap.pre_bytes, &bytes),
                None => "file was deleted during session and restored".into(),
            };
            edits.push(UnauthorizedEdit {
                path: snap.path.clone(),
                diff_preview,
            });
        }
        if let Some(mode) = snap.original_mode
            && matches!(snap.mode, ProtectionMode::ReadOnly)
        {
            set_mode(&snap.path, mode)?;
        }
        Ok(())
    }
}

impl Drop for EnforcementGuard {
    fn drop(&mut self) {
        let Some(snapshots) = self.snapshots.take() else {
            return;
        };
        for snap in snapshots {
            if !matches!(snap.mode, ProtectionMode::ReadOnly) {
                continue;
            }
            let Some(mode) = snap.original_mode else {
                continue;
            };
            if !snap.path.exists() {
                continue;
            }
            let _ = set_mode(&snap.path, mode);
        }
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

fn remove_path(path: &Path) -> MethodologyResult<()> {
    if path.is_dir() {
        std::fs::remove_dir_all(path).map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    } else {
        std::fs::remove_file(path).map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn read_optional_bytes(path: &Path) -> MethodologyResult<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn restore_snapshot_contents(snap: &FileSnapshot) -> MethodologyResult<()> {
    if snap.path.exists() && matches!(snap.mode, ProtectionMode::ReadOnly) {
        set_mode(&snap.path, 0o644)?;
    } else if let Some(parent) = snap.path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(&snap.path, &snap.pre_bytes).map_err(|source| MethodologyError::Io {
        path: snap.path.clone(),
        source,
    })?;
    Ok(())
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

    #[cfg(unix)]
    #[test]
    fn drop_restores_permissions_when_not_finalized() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("tempdir");
        let file = temp.path().join("plan.md");
        std::fs::write(&file, "seed\n").expect("seed");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644))
            .expect("set initial perms");

        let guard = EnforcementGuard::enter(&[ProtectedPath {
            path: file.clone(),
            mode: ProtectionMode::ReadOnly,
        }])
        .expect("enter");
        let mode_during = std::fs::metadata(&file).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode_during, 0o444);

        drop(guard);
        let mode_after = std::fs::metadata(&file).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode_after, 0o644);
    }
}
