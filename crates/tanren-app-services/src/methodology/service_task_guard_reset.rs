//! Guard-reset task mutation used by investigate remediation loops.

use serde::Serialize;
use tanren_contract::methodology::{AckResponse, ResetTaskGuardsParams};
use tanren_domain::TaskId;
use tanren_domain::methodology::capability::{CapabilityScope, ToolCapability};
use tanren_domain::methodology::event_tool::PhaseEventOriginKind;
use tanren_domain::methodology::events::{MethodologyEvent, TaskGuardsReset};
use tanren_domain::methodology::phase_id::PhaseId;
use tanren_domain::methodology::task::{TaskGuardFlags, TaskStatus};

use super::capabilities::enforce;
use super::errors::{MethodologyResult, require_non_empty};
use super::phase_events::PhaseEventAttribution;
use super::service::MethodologyService;

impl MethodologyService {
    /// Reset all guard flags on an implemented task before remediation.
    ///
    /// # Errors
    /// See [`super::errors::MethodologyError`].
    pub async fn reset_task_guards(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        task_id: TaskId,
        reason: String,
        idempotency_key: Option<String>,
    ) -> MethodologyResult<AckResponse> {
        enforce(scope, ToolCapability::TaskComplete, phase)?;
        let spec_id = self.resolve_spec_for_task(task_id).await?;
        let payload = ResetTaskGuardsMutationPayload {
            task_id,
            reason: reason.clone(),
            idempotency_key: idempotency_key.clone(),
        };
        self.run_idempotent_mutation(
            "reset_task_guards",
            spec_id,
            idempotency_key.clone(),
            &payload,
            || async {
                let reason = require_non_empty("/reason", &reason, Some(500))?;
                let status = self.current_task_status(spec_id, task_id).await?;
                let TaskStatus::Implemented { guards } = status else {
                    return Ok(AckResponse::current());
                };
                if !guard_flags_any_set(&guards) {
                    return Ok(AckResponse::current());
                }
                let tool_call_id = idempotency_key
                    .clone()
                    .unwrap_or_else(|| format!("reset_task_guards:{task_id}"));
                self.emit_with_attribution(
                    phase,
                    MethodologyEvent::TaskGuardsReset(TaskGuardsReset {
                        task_id,
                        spec_id,
                        reason,
                        idempotency_key: idempotency_key.clone(),
                    }),
                    PhaseEventAttribution {
                        caused_by_tool_call_id: Some(tool_call_id),
                        origin_kind: Some(PhaseEventOriginKind::ToolPrimary),
                        tool: None,
                    },
                )
                .await?;
                Ok(AckResponse::current())
            },
        )
        .await
    }

    /// Param-struct wrapper so transports can dispatch from a single
    /// compile-time registry.
    pub async fn reset_task_guards_with_params(
        &self,
        scope: &CapabilityScope,
        phase: &PhaseId,
        params: ResetTaskGuardsParams,
    ) -> MethodologyResult<AckResponse> {
        self.reset_task_guards(
            scope,
            phase,
            params.task_id,
            params.reason,
            params.idempotency_key,
        )
        .await
    }
}

#[derive(Debug, Clone, Serialize)]
struct ResetTaskGuardsMutationPayload {
    task_id: TaskId,
    reason: String,
    idempotency_key: Option<String>,
}

fn guard_flags_any_set(guards: &TaskGuardFlags) -> bool {
    guards.gate_checked
        || guards.audited
        || guards.adherent
        || guards.extra.values().any(|flag| *flag)
}
