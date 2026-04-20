//! Shared mutation-session enforcement pipeline used by CLI + MCP.
//!
//! This module wires the three-layer protection from `enforcement.rs`
//! into transport runtime flows:
//! - session enter: snapshot + chmod protected artifacts
//! - session exit: revert unauthorized edits + emit events
//! - postflight: validate evidence frontmatter schema + emit events

use std::path::Path;

use serde::Deserialize;
use tanren_domain::methodology::events::{
    EvidenceSchemaError, MethodologyEvent, UnauthorizedArtifactEdit,
};
use tanren_domain::methodology::evidence::{
    AuditFrontmatter, DemoFrontmatter, InvestigationReport, SignpostsFrontmatter, SpecFrontmatter,
};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::{EventId, NonEmptyString, SpecId};

use super::enforcement::{EnforcementGuard, ProtectedPath, ProtectionMode};
use super::errors::{MethodologyError, MethodologyResult};
use super::service::MethodologyService;

const GENERATED_ARTIFACT_MANIFEST_FILE: &str = ".tanren-generated-artifacts.json";

#[derive(Debug, Deserialize)]
struct GeneratedArtifactManifest {
    #[serde(default)]
    generated_artifacts: Vec<String>,
}

/// Start a mutation session over orchestrator-owned protected artifacts.
///
/// Returns `Ok(None)` when no protected artifacts exist in the spec folder.
///
/// # Errors
/// Returns [`MethodologyError::Io`] on snapshot/chmod failure.
pub fn enter_mutation_session(spec_folder: &Path) -> MethodologyResult<Option<EnforcementGuard>> {
    let protected = protected_artifacts(spec_folder);
    if protected.is_empty() {
        return Ok(None);
    }
    Ok(Some(EnforcementGuard::enter(&protected)?))
}

/// Finalize one mutation session:
/// 1) verify/revert protected files and emit `UnauthorizedArtifactEdit`
/// 2) validate evidence schemas and emit `EvidenceSchemaError` on failure
///
/// # Errors
/// Returns the first typed enforcement/schema error encountered.
pub async fn finalize_mutation_session(
    service: &MethodologyService,
    phase: &PhaseId,
    spec_id: SpecId,
    spec_folder: &Path,
    agent_session_id: &str,
    guard: Option<EnforcementGuard>,
) -> MethodologyResult<()> {
    let agent_session_id =
        NonEmptyString::try_new(agent_session_id).map_err(MethodologyError::Domain)?;

    if let Some(guard) = guard {
        let append_expectations =
            projected_phase_event_append_expectations(service, spec_id, spec_folder, &guard)
                .await?;
        let edits = guard.verify_and_exit_with_append_expectations(&append_expectations)?;
        for edit in edits {
            service
                .emit_event(
                    phase,
                    MethodologyEvent::UnauthorizedArtifactEdit(UnauthorizedArtifactEdit {
                        spec_id,
                        phase: phase.clone(),
                        file: edit.path.display().to_string(),
                        diff_preview: edit.diff_preview,
                        agent_session_id: agent_session_id.clone(),
                    }),
                )
                .await?;
        }
    }

    validate_evidence_files(service, phase, spec_id, spec_folder).await
}

async fn projected_phase_event_append_expectations(
    service: &MethodologyService,
    spec_id: SpecId,
    spec_folder: &Path,
    guard: &EnforcementGuard,
) -> MethodologyResult<std::collections::BTreeMap<std::path::PathBuf, Vec<String>>> {
    let phase_events_path = spec_folder.join("phase-events.jsonl");
    let Some(delta) = guard.append_only_delta(&phase_events_path)? else {
        return Ok(std::collections::BTreeMap::new());
    };
    if delta.appended_lines.is_empty() {
        return Ok(std::collections::BTreeMap::new());
    }

    let mut baseline_event_ids = std::collections::HashSet::new();
    for line in &delta.baseline_lines {
        let Some(event_id) = parse_event_id_from_line_json(line) else {
            return Ok(std::collections::BTreeMap::new());
        };
        baseline_event_ids.insert(event_id);
    }

    let mut appended_event_ids = Vec::with_capacity(delta.appended_lines.len());
    let mut seen_append_event_ids = std::collections::HashSet::new();
    for line in &delta.appended_lines {
        let Some(event_id) = parse_event_id_from_line_json(line) else {
            return Ok(std::collections::BTreeMap::new());
        };
        if baseline_event_ids.contains(&event_id) || !seen_append_event_ids.insert(event_id) {
            return Ok(std::collections::BTreeMap::new());
        }
        appended_event_ids.push(event_id);
    }

    let spec_folder_raw = spec_folder.to_string_lossy().to_string();
    let projected = service
        .store()
        .load_projected_phase_event_outbox_by_event_ids(
            spec_id,
            &spec_folder_raw,
            &appended_event_ids,
        )
        .await?;

    let projected_by_id: std::collections::HashMap<EventId, String> = projected
        .into_iter()
        .map(|row| (row.event_id, row.line_json))
        .collect();

    let mut lines = Vec::with_capacity(delta.appended_lines.len());
    for (event_id, observed_line) in appended_event_ids.into_iter().zip(delta.appended_lines) {
        let Some(projected_line) = projected_by_id.get(&event_id) else {
            return Ok(std::collections::BTreeMap::new());
        };
        let Some(projected_event_id) = parse_event_id_from_line_json(projected_line) else {
            return Ok(std::collections::BTreeMap::new());
        };
        if projected_event_id != event_id || projected_line != &observed_line {
            return Ok(std::collections::BTreeMap::new());
        }
        lines.push(projected_line.clone());
    }

    let mut out = std::collections::BTreeMap::new();
    out.insert(phase_events_path, lines);
    Ok(out)
}

fn parse_event_id_from_line_json(line_json: &str) -> Option<EventId> {
    let value: serde_json::Value = serde_json::from_str(line_json).ok()?;
    let event_id_raw = value.get("event_id").and_then(serde_json::Value::as_str)?;
    let event_id_uuid = uuid::Uuid::parse_str(event_id_raw).ok()?;
    Some(EventId::from_uuid(event_id_uuid))
}

fn protected_artifacts(spec_folder: &Path) -> Vec<ProtectedPath> {
    let mut out = vec![
        ProtectedPath {
            path: spec_folder.join("plan.md"),
            mode: ProtectionMode::ReadOnly,
        },
        ProtectedPath {
            path: spec_folder.join("progress.json"),
            mode: ProtectionMode::ReadOnly,
        },
        ProtectedPath {
            path: spec_folder.join("phase-events.jsonl"),
            mode: ProtectionMode::AppendOnly,
        },
    ];
    let manifest_path = spec_folder.join(GENERATED_ARTIFACT_MANIFEST_FILE);
    if manifest_path.exists() {
        out.push(ProtectedPath {
            path: manifest_path.clone(),
            mode: ProtectionMode::ReadOnly,
        });
        if let Some(manifest) = load_generated_artifact_manifest(&manifest_path) {
            for basename in manifest.generated_artifacts {
                let Some(basename) = normalized_basename(&basename) else {
                    continue;
                };
                out.push(ProtectedPath {
                    path: spec_folder.join(basename),
                    mode: ProtectionMode::ReadOnly,
                });
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path).then(a.mode.cmp(&b.mode)));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

fn load_generated_artifact_manifest(path: &Path) -> Option<GeneratedArtifactManifest> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<GeneratedArtifactManifest>(&raw).ok()
}

fn normalized_basename(raw: &str) -> Option<&str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = Path::new(trimmed);
    if path.components().count() != 1 {
        return None;
    }
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .filter(|name| *name == trimmed)
}

async fn validate_evidence_files(
    service: &MethodologyService,
    phase: &PhaseId,
    spec_id: SpecId,
    spec_folder: &Path,
) -> MethodologyResult<()> {
    for file in ["spec.md", "demo.md", "audit.md", "signposts.md"] {
        let path = spec_folder.join(file);
        if !path.exists() {
            continue;
        }
        let raw = std::fs::read_to_string(&path).map_err(|source| MethodologyError::Io {
            path: path.clone(),
            source,
        })?;
        let parse = match file {
            "spec.md" => SpecFrontmatter::parse_from_markdown(&raw).map(|_| ()),
            "demo.md" => DemoFrontmatter::parse_from_markdown(&raw).map(|_| ()),
            "audit.md" => AuditFrontmatter::parse_from_markdown(&raw).map(|_| ()),
            "signposts.md" => SignpostsFrontmatter::parse_from_markdown(&raw).map(|_| ()),
            _ => Ok(()),
        };
        if let Err(err) = parse {
            let reason = err.to_string();
            emit_evidence_schema_error(service, phase, spec_id, file, &reason).await?;
            return Err(MethodologyError::EvidenceSchema {
                file: file.to_owned(),
                reason,
            });
        }
    }

    let investigation = spec_folder.join("investigation-report.json");
    if investigation.exists() {
        let raw =
            std::fs::read_to_string(&investigation).map_err(|source| MethodologyError::Io {
                path: investigation.clone(),
                source,
            })?;
        if let Err(err) = serde_json::from_str::<InvestigationReport>(&raw) {
            let reason = err.to_string();
            emit_evidence_schema_error(
                service,
                phase,
                spec_id,
                "investigation-report.json",
                &reason,
            )
            .await?;
            return Err(MethodologyError::EvidenceSchema {
                file: "investigation-report.json".into(),
                reason,
            });
        }
    }
    Ok(())
}

async fn emit_evidence_schema_error(
    service: &MethodologyService,
    phase: &PhaseId,
    spec_id: SpecId,
    file: &str,
    reason: &str,
) -> MethodologyResult<()> {
    service
        .emit_event(
            phase,
            MethodologyEvent::EvidenceSchemaError(EvidenceSchemaError {
                spec_id,
                phase: phase.clone(),
                file: file.to_owned(),
                error: NonEmptyString::try_new(reason).map_err(MethodologyError::Domain)?,
            }),
        )
        .await
}

#[cfg(test)]
mod tests;
