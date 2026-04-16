//! `CreateDispatchParams` / `DispatchView` <-> `dispatch_projection::Model`
//! converters.

use sea_orm::ActiveValue::Set;
use tanren_domain::{
    ActorContext, DispatchId, DispatchMode, DispatchSnapshot, DispatchStatus, DispatchView,
    GraphRevision, Lane, Outcome,
};

use crate::entity::dispatch_projection;
use crate::entity::enums::{DispatchStatusModel, LaneModel, OutcomeModel};
use crate::errors::StoreError;
use crate::params::CreateDispatchParams;

/// Build the initial projection row for a newly-created dispatch.
pub(crate) fn params_to_active_model(
    params: &CreateDispatchParams,
) -> Result<dispatch_projection::ActiveModel, StoreError> {
    let dispatch_value = serde_json::to_value(&params.dispatch)?;
    let actor_value = serde_json::to_value(&params.actor)?;

    let graph_revision =
        i32::try_from(params.graph_revision.get()).map_err(|_| StoreError::Conversion {
            context: "dispatch::params_to_active_model",
            reason: "graph_revision exceeds i32::MAX".to_owned(),
        })?;

    Ok(dispatch_projection::ActiveModel {
        dispatch_id: Set(params.dispatch_id.into_uuid()),
        mode: Set(params.mode.to_string()),
        status: Set(DispatchStatusModel::Pending),
        outcome: Set(None),
        lane: Set(LaneModel::from(params.lane)),
        dispatch: Set(dispatch_value),
        actor: Set(actor_value),
        graph_revision: Set(graph_revision),
        user_id: Set(params.actor.user_id.into_uuid()),
        org_id: Set(params.actor.org_id.into_uuid()),
        scope_project_id: Set(params
            .actor
            .project_id
            .map(tanren_domain::ProjectId::into_uuid)),
        scope_team_id: Set(params.actor.team_id.map(tanren_domain::TeamId::into_uuid)),
        scope_api_key_id: Set(params
            .actor
            .api_key_id
            .map(tanren_domain::ApiKeyId::into_uuid)),
        project: Set(params.dispatch.project.as_str().to_owned()),
        created_at: Set(params.created_at),
        updated_at: Set(params.created_at),
    })
}

/// Read a projection row back into the domain [`DispatchView`].
pub(crate) fn model_to_view(model: dispatch_projection::Model) -> Result<DispatchView, StoreError> {
    let mode = parse_mode(&model.mode)?;
    let status = DispatchStatus::from(model.status);
    let outcome = model.outcome.map(Outcome::from);
    let lane = Lane::from(model.lane);

    let dispatch: DispatchSnapshot =
        serde_json::from_value(model.dispatch).map_err(|err| StoreError::Conversion {
            context: "dispatch::model_to_view",
            reason: format!("dispatch snapshot deserialize failed: {err}"),
        })?;
    let actor: ActorContext =
        serde_json::from_value(model.actor).map_err(|err| StoreError::Conversion {
            context: "dispatch::model_to_view",
            reason: format!("actor deserialize failed: {err}"),
        })?;

    let graph_revision_u32 =
        u32::try_from(model.graph_revision).map_err(|_| StoreError::Conversion {
            context: "dispatch::model_to_view",
            reason: "graph_revision is negative".to_owned(),
        })?;

    Ok(DispatchView {
        dispatch_id: DispatchId::from_uuid(model.dispatch_id),
        mode,
        status,
        outcome,
        lane,
        dispatch: Box::new(dispatch),
        actor,
        graph_revision: GraphRevision::new(graph_revision_u32),
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

pub(crate) fn parse_mode(value: &str) -> Result<DispatchMode, StoreError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|err| {
        StoreError::Conversion {
            context: "dispatch::parse_mode",
            reason: format!("unknown dispatch mode `{value}`: {err}"),
        }
    })
}

impl From<DispatchStatus> for DispatchStatusModel {
    fn from(value: DispatchStatus) -> Self {
        match value {
            DispatchStatus::Pending => Self::Pending,
            DispatchStatus::Running => Self::Running,
            DispatchStatus::Completed => Self::Completed,
            DispatchStatus::Failed => Self::Failed,
            DispatchStatus::Cancelled => Self::Cancelled,
        }
    }
}

impl From<DispatchStatusModel> for DispatchStatus {
    fn from(value: DispatchStatusModel) -> Self {
        match value {
            DispatchStatusModel::Pending => Self::Pending,
            DispatchStatusModel::Running => Self::Running,
            DispatchStatusModel::Completed => Self::Completed,
            DispatchStatusModel::Failed => Self::Failed,
            DispatchStatusModel::Cancelled => Self::Cancelled,
        }
    }
}

impl From<Lane> for LaneModel {
    fn from(value: Lane) -> Self {
        match value {
            Lane::Impl => Self::Impl,
            Lane::Audit => Self::Audit,
            Lane::Gate => Self::Gate,
        }
    }
}

impl From<LaneModel> for Lane {
    fn from(value: LaneModel) -> Self {
        match value {
            LaneModel::Impl => Self::Impl,
            LaneModel::Audit => Self::Audit,
            LaneModel::Gate => Self::Gate,
        }
    }
}

impl From<Outcome> for OutcomeModel {
    fn from(value: Outcome) -> Self {
        match value {
            Outcome::Success => Self::Success,
            Outcome::Fail => Self::Fail,
            Outcome::Blocked => Self::Blocked,
            Outcome::Error => Self::Error,
            Outcome::Timeout => Self::Timeout,
        }
    }
}

impl From<OutcomeModel> for Outcome {
    fn from(value: OutcomeModel) -> Self {
        match value {
            OutcomeModel::Success => Self::Success,
            OutcomeModel::Fail => Self::Fail,
            OutcomeModel::Blocked => Self::Blocked,
            OutcomeModel::Error => Self::Error,
            OutcomeModel::Timeout => Self::Timeout,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_round_trip() {
        for mode in [DispatchMode::Auto, DispatchMode::Manual] {
            assert_eq!(parse_mode(&mode.to_string()).expect("parse"), mode);
        }
    }

    #[test]
    fn dispatch_status_round_trip() {
        for status in [
            DispatchStatus::Pending,
            DispatchStatus::Running,
            DispatchStatus::Completed,
            DispatchStatus::Failed,
            DispatchStatus::Cancelled,
        ] {
            let db = DispatchStatusModel::from(status);
            assert_eq!(DispatchStatus::from(db), status);
        }
    }

    #[test]
    fn lane_round_trip() {
        for lane in [Lane::Impl, Lane::Audit, Lane::Gate] {
            let db = LaneModel::from(lane);
            assert_eq!(Lane::from(db), lane);
        }
    }

    #[test]
    fn outcome_round_trip() {
        for outcome in [
            Outcome::Success,
            Outcome::Fail,
            Outcome::Blocked,
            Outcome::Error,
            Outcome::Timeout,
        ] {
            let db = OutcomeModel::from(outcome);
            assert_eq!(Outcome::from(db), outcome);
        }
    }
}
