//! Step-projection converters.

use sea_orm::ActiveValue::Set;
use tanren_domain::{
    DispatchId, GraphRevision, Lane, StepId, StepPayload, StepReadyState, StepResult, StepStatus,
    StepType, StepView,
};

use crate::entity::enums::{LaneModel, StepReadyStateModel, StepStatusModel, StepTypeModel};
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
        step_type: Set(StepTypeModel::from(params.step_type)),
        step_sequence: Set(step_sequence),
        lane: Set(params.lane.map(LaneModel::from)),
        status: Set(StepStatusModel::Pending),
        ready_state: Set(StepReadyStateModel::from(params.ready_state)),
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
    let step_type = StepType::from(model.step_type);
    let status = StepStatus::from(model.status);
    let ready_state = StepReadyState::from(model.ready_state);
    let lane = model.lane.map(Lane::from);

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
/// path.
pub(crate) fn model_to_queued_step(
    model: step_projection::Model,
) -> Result<QueuedStep, StoreError> {
    let step_type = StepType::from(model.step_type);
    let lane = model.lane.map(Lane::from);
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

impl From<StepType> for StepTypeModel {
    fn from(value: StepType) -> Self {
        match value {
            StepType::Provision => Self::Provision,
            StepType::Execute => Self::Execute,
            StepType::Teardown => Self::Teardown,
            StepType::DryRun => Self::DryRun,
        }
    }
}

impl From<StepTypeModel> for StepType {
    fn from(value: StepTypeModel) -> Self {
        match value {
            StepTypeModel::Provision => Self::Provision,
            StepTypeModel::Execute => Self::Execute,
            StepTypeModel::Teardown => Self::Teardown,
            StepTypeModel::DryRun => Self::DryRun,
        }
    }
}

impl From<StepStatus> for StepStatusModel {
    fn from(value: StepStatus) -> Self {
        match value {
            StepStatus::Pending => Self::Pending,
            StepStatus::Running => Self::Running,
            StepStatus::Completed => Self::Completed,
            StepStatus::Failed => Self::Failed,
            StepStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<StepStatusModel> for StepStatus {
    fn from(value: StepStatusModel) -> Self {
        match value {
            StepStatusModel::Pending => Self::Pending,
            StepStatusModel::Running => Self::Running,
            StepStatusModel::Completed => Self::Completed,
            StepStatusModel::Failed => Self::Failed,
            StepStatusModel::Cancelled => Self::Cancelled,
        }
    }
}

impl From<StepReadyState> for StepReadyStateModel {
    fn from(value: StepReadyState) -> Self {
        match value {
            StepReadyState::Blocked => Self::Blocked,
            StepReadyState::Ready => Self::Ready,
        }
    }
}

impl From<StepReadyStateModel> for StepReadyState {
    fn from(value: StepReadyStateModel) -> Self {
        match value {
            StepReadyStateModel::Blocked => Self::Blocked,
            StepReadyStateModel::Ready => Self::Ready,
        }
    }
}
