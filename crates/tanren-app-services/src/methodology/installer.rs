//! Installer — plan, apply, and drift-check the full render cycle.
//!
//! `plan_install` loads the source catalog, renders each command
//! against the per-target format driver, and returns an
//! [`InstallPlan`] enumerating every planned write. `apply_install`
//! writes the plan to disk atomically (tempfile+rename). `dry_run`
//! returns the plan without writing; `strict_dry_run` additionally
//! diffs the plan against current filesystem state and returns drift.

use std::collections::HashMap;
use std::path::PathBuf;

use super::config::{InstallFormat, InstallTarget, MergePolicy, MethodologyConfig};
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
    let (rendered, _refs) = render_catalog(commands, context)?;
    let mut writes = Vec::new();
    for target in &cfg.install_targets {
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
    Ok(written)
}

/// Diff a plan against the current filesystem. Non-empty result
/// indicates drift — `tanren install --strict --dry-run` exits with
/// code 3 if this returns any entries.
#[must_use]
pub fn drift(plan: &InstallPlan) -> Vec<DriftEntry> {
    let mut out = Vec::new();
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_plan_has_no_drift() {
        let plan = InstallPlan { writes: vec![] };
        assert!(drift(&plan).is_empty());
    }
}
