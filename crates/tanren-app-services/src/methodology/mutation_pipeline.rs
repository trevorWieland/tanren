//! Shared mutation-session enforcement pipeline used by CLI + MCP.
//!
//! This module wires the three-layer protection from `enforcement.rs`
//! into transport runtime flows:
//! - session enter: snapshot + chmod protected artifacts
//! - session exit: revert unauthorized edits + emit events
//! - postflight: validate evidence frontmatter schema + emit events

use std::path::{Path, PathBuf};

use tanren_domain::methodology::events::{
    EvidenceSchemaError, MethodologyEvent, UnauthorizedArtifactEdit,
};
use tanren_domain::methodology::evidence::{
    AuditFrontmatter, DemoFrontmatter, InvestigationReport, SignpostsFrontmatter, SpecFrontmatter,
};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::{NonEmptyString, SpecId};

use super::enforcement::EnforcementGuard;
use super::errors::{MethodologyError, MethodologyResult};
use super::service::MethodologyService;

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
    spec_folder: &Path,
    agent_session_id: &str,
    guard: Option<EnforcementGuard>,
) -> MethodologyResult<()> {
    let spec_id = infer_spec_id(spec_folder)?;
    let agent_session_id =
        NonEmptyString::try_new(agent_session_id).map_err(MethodologyError::Domain)?;

    if let Some(guard) = guard {
        let edits = guard.verify_and_exit()?;
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

fn protected_artifacts(spec_folder: &Path) -> Vec<PathBuf> {
    let mut out = vec![
        spec_folder.join("plan.md"),
        spec_folder.join("progress.json"),
    ];
    if let Ok(entries) = std::fs::read_dir(spec_folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
                continue;
            };
            if name.contains("index")
                && path
                    .extension()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("json")
                    })
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out.dedup();
    out
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

fn infer_spec_id(spec_folder: &Path) -> MethodologyResult<SpecId> {
    let hay = spec_folder.to_string_lossy();
    if let Some(spec_id) = find_uuid_like(&hay).map(SpecId::from_uuid) {
        return Ok(spec_id);
    }
    let spec_md = spec_folder.join("spec.md");
    if spec_md.exists() {
        let raw = std::fs::read_to_string(&spec_md).map_err(|source| MethodologyError::Io {
            path: spec_md.clone(),
            source,
        })?;
        let (fm, _) = SpecFrontmatter::parse_from_markdown(&raw).map_err(|e| {
            MethodologyError::Validation(format!("parsing {}: {e}", spec_md.display()))
        })?;
        return Ok(fm.spec_id);
    }
    Err(MethodologyError::Validation(format!(
        "cannot infer spec_id from spec folder `{}` (no UUID-like segment, no parseable spec.md)",
        spec_folder.display()
    )))
}

fn find_uuid_like(input: &str) -> Option<uuid::Uuid> {
    if input.len() < 36 {
        return None;
    }
    for start in 0..=input.len() - 36 {
        let Some(candidate) = input.get(start..start + 36) else {
            continue;
        };
        if let Ok(id) = uuid::Uuid::parse_str(candidate) {
            return Some(id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use tanren_domain::events::DomainEvent;
    use tanren_domain::methodology::events::MethodologyEvent;
    use tanren_store::{EventFilter, EventStore, Store};

    async fn mk_service() -> MethodologyService {
        let store = Store::open_and_migrate("sqlite::memory:")
            .await
            .expect("open");
        let runtime = crate::methodology::service::PhaseEventsRuntime {
            spec_folder: std::env::temp_dir().join(format!(
                "tanren-methodology-mutation-pipeline-{}",
                uuid::Uuid::now_v7()
            )),
            agent_session_id: "test-session".into(),
        };
        MethodologyService::with_runtime(Arc::new(store), vec![], Some(runtime), vec![])
    }

    #[tokio::test]
    async fn finalize_emits_unauthorized_edit_and_reverts_file() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let service = mk_service().await;
        let spec_id = SpecId::new();
        let root = tempfile::tempdir().expect("tempdir");
        let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
        std::fs::create_dir_all(&spec_folder).expect("mkdir");
        let plan = spec_folder.join("plan.md");
        std::fs::write(&plan, "original\n").expect("seed");

        let guard = enter_mutation_session(&spec_folder).expect("enter");
        #[cfg(unix)]
        std::fs::set_permissions(&plan, std::fs::Permissions::from_mode(0o644))
            .expect("unlock protected file to simulate unauthorized agent edit");
        std::fs::write(&plan, "mutated\n").expect("mutate");
        let phase = PhaseId::try_new("do-task").expect("phase");

        finalize_mutation_session(&service, &phase, &spec_folder, "session-1", guard)
            .await
            .expect("finalize");

        let on_disk = std::fs::read_to_string(&plan).expect("read");
        assert_eq!(on_disk, "original\n", "postflight must revert edits");

        let events = service
            .store()
            .query_events(&EventFilter {
                event_type: Some("methodology".into()),
                limit: 100,
                ..EventFilter::new()
            })
            .await
            .expect("query");
        assert!(events.events.into_iter().any(|env| matches!(
            env.payload,
            DomainEvent::Methodology {
                event: MethodologyEvent::UnauthorizedArtifactEdit(_)
            }
        )));
    }

    #[tokio::test]
    async fn finalize_reverts_newly_created_protected_artifact() {
        let service = mk_service().await;
        let spec_id = SpecId::new();
        let root = tempfile::tempdir().expect("tempdir");
        let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
        std::fs::create_dir_all(&spec_folder).expect("mkdir");

        let guard = enter_mutation_session(&spec_folder).expect("enter");
        let created = spec_folder.join("plan.md");
        std::fs::write(&created, "created during session\n").expect("create");
        let phase = PhaseId::try_new("do-task").expect("phase");

        finalize_mutation_session(&service, &phase, &spec_folder, "session-3", guard)
            .await
            .expect("finalize");

        assert!(
            !created.exists(),
            "postflight must remove newly created protected artifacts"
        );

        let events = service
            .store()
            .query_events(&EventFilter {
                event_type: Some("methodology".into()),
                limit: 100,
                ..EventFilter::new()
            })
            .await
            .expect("query");
        assert!(events.events.into_iter().any(|env| matches!(
            env.payload,
            DomainEvent::Methodology {
                event: MethodologyEvent::UnauthorizedArtifactEdit(_)
            }
        )));
    }

    #[tokio::test]
    async fn finalize_reverts_newly_created_protected_index_artifact() {
        let service = mk_service().await;
        let spec_id = SpecId::new();
        let root = tempfile::tempdir().expect("tempdir");
        let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
        std::fs::create_dir_all(&spec_folder).expect("mkdir");

        let guard = enter_mutation_session(&spec_folder).expect("enter");
        let created = spec_folder.join("tool-index.json");
        std::fs::write(&created, "{\"generated\":true}\n").expect("create");
        let phase = PhaseId::try_new("do-task").expect("phase");

        finalize_mutation_session(&service, &phase, &spec_folder, "session-4", guard)
            .await
            .expect("finalize");

        assert!(
            !created.exists(),
            "postflight must remove newly created protected index artifacts"
        );

        let events = service
            .store()
            .query_events(&EventFilter {
                event_type: Some("methodology".into()),
                limit: 100,
                ..EventFilter::new()
            })
            .await
            .expect("query");
        assert!(events.events.into_iter().any(|env| matches!(
            env.payload,
            DomainEvent::Methodology {
                event: MethodologyEvent::UnauthorizedArtifactEdit(_)
            }
        )));
    }

    #[tokio::test]
    async fn malformed_evidence_emits_schema_error_and_fails_closed() {
        let service = mk_service().await;
        let spec_id = SpecId::new();
        let root = tempfile::tempdir().expect("tempdir");
        let spec_folder = root.path().join(format!("2026-01-01-0101-{spec_id}-demo"));
        std::fs::create_dir_all(&spec_folder).expect("mkdir");
        std::fs::write(spec_folder.join("audit.md"), "not-frontmatter\n").expect("write");
        let phase = PhaseId::try_new("audit-task").expect("phase");

        let err = finalize_mutation_session(&service, &phase, &spec_folder, "session-2", None)
            .await
            .expect_err("malformed evidence must fail");
        assert!(matches!(err, MethodologyError::EvidenceSchema { .. }));

        let events = service
            .store()
            .query_events(&EventFilter {
                event_type: Some("methodology".into()),
                limit: 100,
                ..EventFilter::new()
            })
            .await
            .expect("query");
        assert!(events.events.into_iter().any(|env| matches!(
            env.payload,
            DomainEvent::Methodology {
                event: MethodologyEvent::EvidenceSchemaError(_)
            }
        )));
    }
}
