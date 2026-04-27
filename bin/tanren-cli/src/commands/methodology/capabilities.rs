use anyhow::Result;
use clap::Args;
use serde::Serialize;
use tanren_app_services::methodology::{
    KnownPhase, MethodologyError, PhaseId, default_phase_capability_bindings,
};

#[derive(Debug, Clone, Args)]
pub(crate) struct PhaseCapabilitiesArgs {
    /// Optional phase filter (`do-task`, `audit-spec`, ...).
    #[arg(long)]
    pub phase: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PhaseCapabilitiesResponse {
    schema_version: &'static str,
    phases: Vec<PhaseCapabilityRow>,
}

#[derive(Debug, Clone, Serialize)]
struct PhaseCapabilityRow {
    phase: String,
    capabilities: Vec<String>,
    capabilities_csv: String,
}

pub(crate) fn render_phase_capabilities(
    args: PhaseCapabilitiesArgs,
) -> Result<PhaseCapabilitiesResponse, MethodologyError> {
    let requested = if let Some(raw) = args.phase {
        if raw == "cli-admin" {
            None
        } else {
            let phase =
                PhaseId::try_new(raw.clone()).map_err(|err| MethodologyError::FieldValidation {
                    field_path: "/phase".into(),
                    expected: "non-empty phase identifier".into(),
                    actual: raw,
                    remediation: err.to_string(),
                })?;
            Some(phase)
        }
    } else {
        None
    };
    let mut rows = Vec::new();
    for binding in default_phase_capability_bindings() {
        if let Some(phase) = requested.as_ref()
            && phase.known().is_none_or(|known| known != binding.phase)
        {
            continue;
        }
        let capabilities = binding
            .capabilities
            .iter()
            .map(|cap| cap.tag().to_owned())
            .collect::<Vec<_>>();
        rows.push(PhaseCapabilityRow {
            capabilities_csv: capabilities.join(","),
            capabilities,
            phase: binding.phase.tag().to_owned(),
        });
    }
    if let Some(phase) = requested
        && phase.known().is_none()
    {
        return Err(MethodologyError::FieldValidation {
            field_path: "/phase".into(),
            expected: "known built-in phase".into(),
            actual: phase.as_str().to_owned(),
            remediation: format!(
                "use one of: {}",
                KnownPhase::all()
                    .iter()
                    .map(|item| item.tag())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }
    Ok(PhaseCapabilitiesResponse {
        schema_version: "1.0.0",
        phases: rows,
    })
}
