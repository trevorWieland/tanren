//! Step-projection converters.
//!
//! Covers both the projection-row direction (domain → `ActiveModel` for
//! inserts / Model → `StepView` for reads) and the queue direction
//! (Model → [`QueuedStep`](crate::params::QueuedStep) for dequeue).
//! Every enum column round-trips through exhaustive helpers defined
//! here rather than through the serde string path, so the
//! `snake_case` spelling is authoritative and checked at compile time.

use sea_orm::ActiveValue::Set;
use tanren_domain::{
    DispatchId, GraphRevision, StepId, StepPayload, StepReadyState, StepResult, StepStatus,
    StepType, StepView,
};

use crate::entity::step_projection;
use crate::errors::StoreError;
use crate::params::{EnqueueStepParams, QueuedStep};

/// Build an [`step_projection::ActiveModel`] from
/// [`EnqueueStepParams`] ready for insert.
pub(crate) fn enqueue_to_active_model(
    params: &EnqueueStepParams,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<step_projection::ActiveModel, StoreError> {
    let payload_value = serde_json::to_value(&params.payload)?;
    let depends_on_value = serde_json::to_value(&params.depends_on)?;
    let graph_revision =
        i32::try_from(params.graph_revision.get()).map_err(|_| StoreError::Conversion {
            context: "step::enqueue_to_active_model",
            reason: "graph_revision exceeds i32::MAX".to_owned(),
        })?;
    let step_sequence =
        i32::try_from(params.step_sequence).map_err(|_| StoreError::Conversion {
            context: "step::enqueue_to_active_model",
            reason: "step_sequence exceeds i32::MAX".to_owned(),
        })?;

    Ok(step_projection::ActiveModel {
        step_id: Set(params.step_id.into_uuid()),
        dispatch_id: Set(params.dispatch_id.into_uuid()),
        step_type: Set(params.step_type.to_string()),
        step_sequence: Set(step_sequence),
        lane: Set(params.lane.map(|l| l.to_string())),
        status: Set(StepStatus::Pending.to_string()),
        ready_state: Set(params.ready_state.to_string()),
        depends_on: Set(depends_on_value),
        graph_revision: Set(graph_revision),
        worker_id: Set(None),
        payload: Set(Some(payload_value)),
        result: Set(None),
        error: Set(None),
        retry_count: Set(0),
        last_heartbeat_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    })
}

/// Read a projection row back into the domain [`StepView`].
pub(crate) fn model_to_view(model: step_projection::Model) -> Result<StepView, StoreError> {
    let step_type = parse_step_type(&model.step_type)?;
    let status = parse_step_status(&model.status)?;
    let ready_state = parse_ready_state(&model.ready_state)?;
    let lane = model
        .lane
        .as_deref()
        .map(super::dispatch::parse_lane)
        .transpose()?;

    let depends_on: Vec<StepId> =
        serde_json::from_value(model.depends_on).map_err(|err| StoreError::Conversion {
            context: "step::model_to_view",
            reason: format!("depends_on deserialize failed: {err}"),
        })?;

    let payload = model
        .payload
        .map(serde_json::from_value::<StepPayload>)
        .transpose()
        .map_err(|err| StoreError::Conversion {
            context: "step::model_to_view",
            reason: format!("payload deserialize failed: {err}"),
        })?;

    let result = model
        .result
        .map(serde_json::from_value::<StepResult>)
        .transpose()
        .map_err(|err| StoreError::Conversion {
            context: "step::model_to_view",
            reason: format!("result deserialize failed: {err}"),
        })?;

    let graph_revision_u32 =
        u32::try_from(model.graph_revision).map_err(|_| StoreError::Conversion {
            context: "step::model_to_view",
            reason: "graph_revision is negative".to_owned(),
        })?;
    let step_sequence = u32::try_from(model.step_sequence).map_err(|_| StoreError::Conversion {
        context: "step::model_to_view",
        reason: "step_sequence is negative".to_owned(),
    })?;
    let retry_count = u32::try_from(model.retry_count).map_err(|_| StoreError::Conversion {
        context: "step::model_to_view",
        reason: "retry_count is negative".to_owned(),
    })?;

    Ok(StepView {
        step_id: StepId::from_uuid(model.step_id),
        dispatch_id: DispatchId::from_uuid(model.dispatch_id),
        step_type,
        step_sequence,
        lane,
        status,
        ready_state,
        depends_on,
        graph_revision: GraphRevision::new(graph_revision_u32),
        worker_id: model.worker_id,
        payload,
        result,
        error: model.error,
        retry_count,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

/// Read a projection row into a [`QueuedStep`] shape for the dequeue
/// path. Populated from the same columns as `StepView`, but drops the
/// fields a worker handing off a task does not need (status, result,
/// retry count, timestamps).
pub(crate) fn model_to_queued_step(
    model: step_projection::Model,
) -> Result<QueuedStep, StoreError> {
    let step_type = parse_step_type(&model.step_type)?;
    let lane = model
        .lane
        .as_deref()
        .map(super::dispatch::parse_lane)
        .transpose()?;
    let step_sequence = u32::try_from(model.step_sequence).map_err(|_| StoreError::Conversion {
        context: "step::model_to_queued_step",
        reason: "step_sequence is negative".to_owned(),
    })?;
    let payload = model
        .payload
        .ok_or_else(|| StoreError::Conversion {
            context: "step::model_to_queued_step",
            reason: "queued step missing payload".to_owned(),
        })
        .and_then(|value| {
            serde_json::from_value::<StepPayload>(value).map_err(|err| StoreError::Conversion {
                context: "step::model_to_queued_step",
                reason: format!("payload deserialize failed: {err}"),
            })
        })?;

    Ok(QueuedStep {
        step_id: StepId::from_uuid(model.step_id),
        dispatch_id: DispatchId::from_uuid(model.dispatch_id),
        step_type,
        step_sequence,
        lane,
        payload,
    })
}

// ---------------------------------------------------------------------------
// Enum <-> string helpers
//
// Write path: use the domain `Display` impl directly
// (`enum.to_string()`). Every persisted enum already renders as the
// canonical snake_case tag, so there is no store-local mapping to
// maintain and adding a new domain variant does not require editing
// this file. See S-02 in LANE-0.3-AUDIT.md.
//
// Read path: parse via `serde_json::from_value(Value::String(...))`
// so the same `#[serde(rename_all = "snake_case")]` impl the domain
// uses on the wire is the source of truth.
// ---------------------------------------------------------------------------

pub(crate) fn parse_step_type(value: &str) -> Result<StepType, StoreError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|err| {
        StoreError::Conversion {
            context: "step::parse_step_type",
            reason: format!("unknown step type `{value}`: {err}"),
        }
    })
}

pub(crate) fn parse_step_status(value: &str) -> Result<StepStatus, StoreError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|err| {
        StoreError::Conversion {
            context: "step::parse_step_status",
            reason: format!("unknown step status `{value}`: {err}"),
        }
    })
}

pub(crate) fn parse_ready_state(value: &str) -> Result<StepReadyState, StoreError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|err| {
        StoreError::Conversion {
            context: "step::parse_ready_state",
            reason: format!("unknown ready state `{value}`: {err}"),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_type_round_trip() {
        for kind in [
            StepType::Provision,
            StepType::Execute,
            StepType::Teardown,
            StepType::DryRun,
        ] {
            assert_eq!(parse_step_type(&kind.to_string()).expect("parse"), kind);
        }
    }

    #[test]
    fn step_status_round_trip() {
        for status in [
            StepStatus::Pending,
            StepStatus::Running,
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Cancelled,
        ] {
            assert_eq!(
                parse_step_status(&status.to_string()).expect("parse"),
                status
            );
        }
    }

    #[test]
    fn ready_state_round_trip() {
        for state in [StepReadyState::Blocked, StepReadyState::Ready] {
            assert_eq!(parse_ready_state(&state.to_string()).expect("parse"), state);
        }
    }

    #[test]
    fn parse_step_type_rejects_unknown_variant() {
        let err = parse_step_type("not_a_type").expect_err("should fail");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    #[test]
    fn parse_step_status_rejects_unknown_variant() {
        let err = parse_step_status("not_a_status").expect_err("should fail");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    #[test]
    fn parse_ready_state_rejects_unknown_variant() {
        let err = parse_ready_state("not_a_state").expect_err("should fail");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }
}
