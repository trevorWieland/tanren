//! Installer — plan, apply, and drift-check the full render cycle.
//!
//! `plan_install` loads the source catalog, renders each command
//! against the per-target format driver, and returns an
//! [`InstallPlan`] enumerating every planned write. `apply_install`
//! writes the plan to disk atomically (tempfile+rename). `dry_run`
//! returns the plan without writing; `strict_dry_run` additionally
//! diffs the plan against current filesystem state and returns drift.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use super::config::{InstallBinding, InstallFormat, InstallTarget, MergePolicy, MethodologyConfig};
use super::errors::{MethodologyError, MethodologyResult};
use super::formats::render_commands;
use super::renderer::{RenderedCommand, render_catalog};
use super::source::{CommandSource, load_catalog};

/// A planned install — every file that would be written and under
/// what merge policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    pub writes: Vec<PlannedWrite>,
}

/// One planned file write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWrite {
    pub dest: PathBuf,
    pub bytes: Vec<u8>,
    pub merge_policy: MergePolicy,
    pub format: InstallFormat,
}

/// Drift between a plan and the current filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftEntry {
    pub dest: PathBuf,
    pub reason: DriftReason,
}

/// Why a planned write would deviate from on-disk state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DriftReason {
    Missing,
    Differs,
    ExtraFile,
}

/// Build an install plan from the config plus current source tree.
///
/// # Errors
/// Returns [`MethodologyError`] on source I/O, variable-resolution,
/// or format-render failure.
pub fn plan_install(
    cfg: &MethodologyConfig,
    commands: &[CommandSource],
    context: &HashMap<String, String>,
) -> MethodologyResult<InstallPlan> {
    let mut writes = Vec::new();
    for target in &cfg.install_targets {
        let mut target_context = context.clone();
        target_context.insert(
            "TASK_TOOL_BINDING".into(),
            binding_label(target.binding).to_owned(),
        );
        let (rendered, _refs) = render_catalog(commands, &target_context)?;
        match target.format {
            InstallFormat::ClaudeCode | InstallFormat::CodexSkills | InstallFormat::Opencode => {
                plan_command_target(target, &rendered, &mut writes)?;
            }
            InstallFormat::StandardsBaseline => {
                plan_standards_target(target, &mut writes);
            }
            InstallFormat::ClaudeMcpJson
            | InstallFormat::CodexConfigToml
            | InstallFormat::OpencodeJson => {
                // MCP config writers are driven by `cfg.mcp.also_write_configs`
                // and invoked separately; fall through.
            }
        }
    }
    Ok(InstallPlan { writes })
}

const fn binding_label(binding: InstallBinding) -> &'static str {
    match binding {
        InstallBinding::Mcp => "mcp",
        InstallBinding::Cli => "cli",
        InstallBinding::None => "none",
    }
}

fn plan_command_target(
    target: &InstallTarget,
    rendered: &[RenderedCommand],
    writes: &mut Vec<PlannedWrite>,
) -> MethodologyResult<()> {
    let artifacts = render_commands(rendered, target.format, &target.path)?;
    for art in artifacts {
        writes.push(PlannedWrite {
            dest: art.dest,
            bytes: art.bytes,
            merge_policy: target.merge_policy,
            format: target.format,
        });
    }
    Ok(())
}

/// Render the bundled baseline-standards tree under `target.path`.
/// Each standard writes to `<path>/<category>/<name>.md` with the
/// target's merge policy (typically `preserve_existing`, so adopters'
/// edits never get stomped by reinstall).
fn plan_standards_target(target: &InstallTarget, writes: &mut Vec<PlannedWrite>) {
    for std in super::standards::baseline_standards() {
        let dest = super::standards::standard_path(&target.path, &std);
        let bytes = super::standards::render_standard(&std);
        writes.push(PlannedWrite {
            dest,
            bytes,
            merge_policy: target.merge_policy,
            format: target.format,
        });
    }
}

/// Entry-point wrapper: load the catalog + plan in one step.
///
/// # Errors
/// See [`plan_install`] and [`load_catalog`].
pub fn plan_install_from_root(
    cfg: &MethodologyConfig,
    context: &HashMap<String, String>,
) -> MethodologyResult<InstallPlan> {
    let commands = load_catalog(&cfg.source.path)?;
    plan_install(cfg, &commands, context)
}

/// Apply a plan — per-file tempfile+rename for atomicity. Returns the
/// list of paths written.
///
/// Not transactional across files: a mid-plan failure returns the
/// error after partial writes. Idempotent — rerunning after fixing the
/// cause converges.
///
/// # Errors
/// Returns [`MethodologyError::Io`] on any filesystem failure.
pub fn apply_install(plan: &InstallPlan) -> MethodologyResult<Vec<PathBuf>> {
    let (planned_by_root, destructive_roots) = build_root_plan_index(plan);
    let workspace_root = workspace_root()?;
    for root in destructive_roots {
        let Some(planned) = planned_by_root.get(&root) else {
            continue;
        };
        let _ = validate_safe_destructive_root(&root, planned, &workspace_root)?;
    }

    let mut written = Vec::with_capacity(plan.writes.len());
    for w in &plan.writes {
        if let MergePolicy::PreserveExisting = w.merge_policy
            && w.dest.exists()
        {
            continue;
        }
        if let Some(parent) = w.dest.parent() {
            std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let tmp = w.dest.with_extension("tanren-install-tmp");
        std::fs::write(&tmp, &w.bytes).map_err(|source| MethodologyError::Io {
            path: tmp.clone(),
            source,
        })?;
        std::fs::rename(&tmp, &w.dest).map_err(|source| MethodologyError::Io {
            path: w.dest.clone(),
            source,
        })?;
        written.push(w.dest.clone());
    }
    prune_unmanaged_destructive_files(plan)?;
    Ok(written)
}

/// Diff a plan against the current filesystem. Non-empty result
/// indicates drift — `tanren install --strict --dry-run` exits with
/// code 3 if this returns any entries.
///
/// Scans in two passes:
/// 1. Every planned write is compared to its on-disk contents.
///    Missing / differing → `DriftEntry`.
/// 2. For every `destructive` target directory seen in the plan, the
///    directory is walked; any file on disk that is **not** in the
///    planned set is reported as `ExtraFile`. This catches stale files
///    left over from prior installs. For `preserve_existing`
///    (standards baseline) and `preserve_other_keys` (MCP config)
///    targets, extra-file reporting is skipped by design — the merge
///    policy explicitly permits adopters to keep their own files.
#[must_use]
pub fn drift(plan: &InstallPlan) -> Vec<DriftEntry> {
    let mut out = Vec::new();
    // Pass 1: planned write → Missing | Differs | ok.
    for w in &plan.writes {
        match std::fs::read(&w.dest) {
            Ok(on_disk) => {
                if on_disk != w.bytes {
                    out.push(DriftEntry {
                        dest: w.dest.clone(),
                        reason: DriftReason::Differs,
                    });
                }
            }
            Err(_) => {
                out.push(DriftEntry {
                    dest: w.dest.clone(),
                    reason: DriftReason::Missing,
                });
            }
        }
    }

    // Pass 2: for each destructive target root, flag on-disk files
    // that aren't planned.
    //
    // A "target root" is the common parent directory of the planned
    // writes grouped by (format, merge_policy). We scan below each
    // root and compare to the set of planned destinations.
    let (planned_by_root, destructive_roots) = build_root_plan_index(plan);
    for root in destructive_roots {
        let Some(planned) = planned_by_root.get(&root) else {
            continue;
        };
        walk_files(&root, &mut |found| {
            if !planned.contains(found) {
                out.push(DriftEntry {
                    dest: found.to_path_buf(),
                    reason: DriftReason::ExtraFile,
                });
            }
        });
    }

    out.sort_by(|a, b| a.dest.cmp(&b.dest));
    out
}

fn prune_unmanaged_destructive_files(plan: &InstallPlan) -> MethodologyResult<()> {
    let (planned_by_root, destructive_roots) = build_root_plan_index(plan);
    let workspace_root = workspace_root()?;
    for root in destructive_roots {
        let Some(planned) = planned_by_root.get(&root) else {
            continue;
        };
        let _ = validate_safe_destructive_root(&root, planned, &workspace_root)?;
        let mut remove_error: Option<(PathBuf, std::io::Error)> = None;
        walk_files(&root, &mut |found| {
            if planned.contains(found) {
                return;
            }
            if remove_error.is_some() {
                return;
            }
            if let Err(source) = std::fs::remove_file(found) {
                remove_error = Some((found.to_path_buf(), source));
            }
        });
        if let Some((path, source)) = remove_error {
            return Err(MethodologyError::Io { path, source });
        }
    }
    Ok(())
}

fn validate_safe_destructive_root(
    root: &Path,
    planned: &std::collections::BTreeSet<PathBuf>,
    workspace_root: &Path,
) -> MethodologyResult<PathBuf> {
    if root.as_os_str().is_empty() || root == Path::new(".") || root == Path::new("/") {
        return Err(MethodologyError::Validation(format!(
            "refusing destructive prune on unsafe root `{}`",
            root.display()
        )));
    }
    if path_has_parent_traversal(root) {
        return Err(MethodologyError::Validation(format!(
            "refusing destructive prune on path traversal root `{}`",
            root.display()
        )));
    }

    let resolved_root = resolve_path(root, workspace_root)?;
    if resolved_root == Path::new("/") || resolved_root == workspace_root {
        return Err(MethodologyError::Validation(format!(
            "refusing destructive prune on unsafe root `{}`",
            root.display()
        )));
    }
    if !resolved_root.starts_with(workspace_root) {
        return Err(MethodologyError::Validation(format!(
            "refusing destructive prune on root `{}` that escapes workspace `{}`",
            root.display(),
            workspace_root.display()
        )));
    }

    for dest in planned {
        if !dest.starts_with(root) {
            return Err(MethodologyError::Validation(format!(
                "refusing destructive prune: planned path `{}` is not under root `{}`",
                dest.display(),
                root.display()
            )));
        }
        if path_has_parent_traversal(dest) {
            return Err(MethodologyError::Validation(format!(
                "refusing destructive prune on traversing destination `{}`",
                dest.display()
            )));
        }
        let resolved_dest = resolve_path(dest, workspace_root)?;
        if !resolved_dest.starts_with(&resolved_root) {
            return Err(MethodologyError::Validation(format!(
                "refusing destructive prune: destination `{}` escapes validated root `{}`",
                dest.display(),
                root.display()
            )));
        }
    }

    Ok(resolved_root)
}

fn workspace_root() -> MethodologyResult<PathBuf> {
    let cwd = std::env::current_dir().map_err(|source| MethodologyError::Io {
        path: PathBuf::from("."),
        source,
    })?;
    std::fs::canonicalize(&cwd).map_err(|source| MethodologyError::Io { path: cwd, source })
}

fn resolve_path(path: &Path, workspace_root: &Path) -> MethodologyResult<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };
    canonicalize_allow_missing(&absolute)
}

fn canonicalize_allow_missing(path: &Path) -> MethodologyResult<PathBuf> {
    let mut probe = path.to_path_buf();
    let mut missing: Vec<OsString> = Vec::new();
    loop {
        match std::fs::canonicalize(&probe) {
            Ok(mut canonical) => {
                for seg in missing.iter().rev() {
                    canonical.push(seg);
                }
                return Ok(canonical);
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                let Some(parent) = probe.parent() else {
                    return Err(MethodologyError::Io {
                        path: probe,
                        source,
                    });
                };
                let Some(last) = probe.file_name() else {
                    return Err(MethodologyError::Io {
                        path: probe,
                        source,
                    });
                };
                missing.push(last.to_os_string());
                probe = parent.to_path_buf();
            }
            Err(source) => {
                return Err(MethodologyError::Io {
                    path: probe,
                    source,
                });
            }
        }
    }
}

fn path_has_parent_traversal(path: &Path) -> bool {
    path.components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn build_root_plan_index(
    plan: &InstallPlan,
) -> (
    std::collections::BTreeMap<PathBuf, std::collections::BTreeSet<PathBuf>>,
    std::collections::BTreeSet<PathBuf>,
) {
    use std::collections::{BTreeMap, BTreeSet};
    let mut planned_by_root: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
    let mut destructive_roots: BTreeSet<PathBuf> = BTreeSet::new();
    for w in &plan.writes {
        let root = target_root_for(&w.dest, w.format);
        planned_by_root
            .entry(root.clone())
            .or_default()
            .insert(w.dest.clone());
        if matches!(w.merge_policy, MergePolicy::Destructive) {
            destructive_roots.insert(root);
        }
    }
    (planned_by_root, destructive_roots)
}

/// Derive the scan-root for a planned write. For `codex-skills` the
/// rendered artifact lives in `<root>/<name>/SKILL.md`, so we walk
/// from `<root>`. For every other format the root is the first parent
/// directory.
fn target_root_for(dest: &Path, format: InstallFormat) -> PathBuf {
    match format {
        InstallFormat::CodexSkills => dest
            .parent()
            .and_then(|p| p.parent())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
        _ => dest
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
    }
}

fn walk_files(root: &Path, visit: &mut dyn FnMut(&Path)) {
    if !root.is_dir() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_files(&path, visit);
        } else if path.is_file() {
            // Skip temporary install files from prior in-flight runs.
            if path.extension().is_some_and(|e| e == "tanren-install-tmp") {
                continue;
            }
            visit(&path);
        }
    }
}
