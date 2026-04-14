//! `CreateDispatchParams` / `DispatchView` <-> `dispatch_projection::Model`
//! converters.

use sea_orm::ActiveValue::Set;
use tanren_domain::{
    ActorContext, DispatchId, DispatchMode, DispatchSnapshot, DispatchStatus, DispatchView,
    GraphRevision, Lane, Outcome,
};

use crate::entity::dispatch_projection;
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
        status: Set(DispatchStatus::Pending.to_string()),
        outcome: Set(None),
        lane: Set(params.lane.to_string()),
        dispatch: Set(dispatch_value),
        actor: Set(actor_value),
        graph_revision: Set(graph_revision),
        user_id: Set(params.actor.user_id.into_uuid()),
        org_id: Set(Some(params.actor.org_id.into_uuid())),
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
    let status = parse_status(&model.status)?;
    let outcome = model.outcome.as_deref().map(parse_outcome).transpose()?;
    let lane = parse_lane(&model.lane)?;

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

// ---------------------------------------------------------------------------
// Enum <-> string helpers — domain `Display` is the source of truth
// for the write path, serde is the source of truth for the read path.
// See S-02 in LANE-0.3-AUDIT.md.
// ---------------------------------------------------------------------------

pub(crate) fn parse_mode(value: &str) -> Result<DispatchMode, StoreError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|err| {
        StoreError::Conversion {
            context: "dispatch::parse_mode",
            reason: format!("unknown dispatch mode `{value}`: {err}"),
        }
    })
}

pub(crate) fn parse_status(value: &str) -> Result<DispatchStatus, StoreError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|err| {
        StoreError::Conversion {
            context: "dispatch::parse_status",
            reason: format!("unknown dispatch status `{value}`: {err}"),
        }
    })
}

pub(crate) fn parse_outcome(value: &str) -> Result<Outcome, StoreError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|err| {
        StoreError::Conversion {
            context: "dispatch::parse_outcome",
            reason: format!("unknown outcome `{value}`: {err}"),
        }
    })
}

pub(crate) fn parse_lane(value: &str) -> Result<Lane, StoreError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned())).map_err(|err| {
        StoreError::Conversion {
            context: "dispatch::parse_lane",
            reason: format!("unknown lane `{value}`: {err}"),
        }
    })
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
    fn status_round_trip() {
        for status in [
            DispatchStatus::Pending,
            DispatchStatus::Running,
            DispatchStatus::Completed,
            DispatchStatus::Failed,
            DispatchStatus::Cancelled,
        ] {
            assert_eq!(parse_status(&status.to_string()).expect("parse"), status);
        }
    }

    #[test]
    fn lane_round_trip() {
        for lane in [Lane::Impl, Lane::Audit, Lane::Gate] {
            assert_eq!(parse_lane(&lane.to_string()).expect("parse"), lane);
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
            assert_eq!(parse_outcome(&outcome.to_string()).expect("parse"), outcome);
        }
    }

    #[test]
    fn parse_mode_rejects_unknown_variant() {
        let err = parse_mode("does_not_exist").expect_err("should fail");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    #[test]
    fn parse_status_rejects_unknown_variant() {
        let err = parse_status("not_a_status").expect_err("should fail");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    #[test]
    fn parse_lane_rejects_unknown_variant() {
        let err = parse_lane("not_a_lane").expect_err("should fail");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    #[test]
    fn parse_outcome_rejects_unknown_variant() {
        let err = parse_outcome("not_an_outcome").expect_err("should fail");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }
}
