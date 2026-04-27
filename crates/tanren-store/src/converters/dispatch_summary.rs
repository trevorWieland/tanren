//! Lean `dispatch_projection` row → [`DispatchSummary`] converter.
//!
//! Unlike [`crate::converters::dispatch::model_to_view`], this path
//! deliberately **does not** read or decode the `dispatch` or `actor`
//! JSON columns — the list query selects only scalar columns, so the
//! heavy [`tanren_domain::DispatchSnapshot`] + [`tanren_domain::ActorContext`]
//! deserialize cost is eliminated from paginated reads.

use chrono::{DateTime, Utc};
use tanren_domain::{DispatchId, DispatchStatus, DispatchSummary, Lane, NonEmptyString, Outcome};

use crate::converters::dispatch::parse_mode;
use crate::entity::enums::{DispatchStatusModel, LaneModel, OutcomeModel};
use crate::errors::StoreError;

/// Tuple alias for the exact scalar columns read from
/// `dispatch_projection` during a list query.
pub(crate) type SummaryRow = (
    uuid::Uuid,           // dispatch_id
    String,               // mode tag
    DispatchStatusModel,  // status
    Option<OutcomeModel>, // outcome
    LaneModel,            // lane
    String,               // project
    DateTime<Utc>,        // created_at
    DateTime<Utc>,        // updated_at
);

/// Build a [`DispatchSummary`] from the scalar tuple returned by the
/// summary SELECT. Returns a [`StoreError::Conversion`] only on the
/// two typed-value parses that are still necessary (mode tag and
/// project string) — no JSON is touched.
pub(crate) fn tuple_to_summary(row: SummaryRow) -> Result<DispatchSummary, StoreError> {
    let (
        dispatch_id,
        mode_tag,
        status_model,
        outcome_model,
        lane_model,
        project,
        created_at,
        updated_at,
    ) = row;
    let mode = parse_mode(&mode_tag)?;
    let status = DispatchStatus::from(status_model);
    let outcome = outcome_model.map(Outcome::from);
    let lane = Lane::from(lane_model);
    let project = NonEmptyString::try_new(project).map_err(|err| StoreError::Conversion {
        context: "dispatch_summary::tuple_to_summary",
        reason: format!("invalid project column: {err}"),
    })?;
    Ok(DispatchSummary {
        dispatch_id: DispatchId::from_uuid(dispatch_id),
        mode,
        status,
        outcome,
        lane,
        project,
        created_at,
        updated_at,
    })
}
