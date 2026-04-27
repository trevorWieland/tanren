//! Method-specific envelope validators.
//!
//! Each validator matches on the [`DomainEvent`] variant carried by the
//! params' envelope and compares **every overlapping field** against the
//! params struct that drives the projection write. This guarantees that
//! the event appended co-transactionally with a projection mutation is
//! semantically identical to it — not just routed to the same entity,
//! but carrying the same data.
//!
//! Fields that exist only in the event (e.g. `duration_secs` in
//! `StepCompleted`) and have no counterpart in the params struct are
//! left unchecked — the store has no ground truth to compare them
//! against.

use tanren_domain::DomainEvent;

use super::events::{dispatch_status_event_tag, validate_routing};
use crate::errors::StoreError;
use crate::params::{
    AckAndEnqueueParams, AckParams, CreateDispatchParams, EnqueueStepParams, NackParams,
    UpdateDispatchStatusParams,
};

#[path = "validate_cancel_dispatch.rs"]
mod validate_cancel_dispatch;
#[path = "validate_create_with_initial_step.rs"]
mod validate_create_with_initial_step;
pub(crate) use self::validate_cancel_dispatch::validate_cancel_dispatch;
pub(crate) use self::validate_create_with_initial_step::validate_create_dispatch_with_initial_step;

/// Validate that an [`EnqueueStepParams`]'s envelope carries a
/// `StepEnqueued` payload whose fields match the params that build the
/// projection row.
pub(crate) fn validate_enqueue_step(params: &EnqueueStepParams) -> Result<(), StoreError> {
    validate_routing(&params.enqueue_event)?;
    match &params.enqueue_event.payload {
        DomainEvent::StepEnqueued {
            dispatch_id,
            step_id,
            step_type,
            step_sequence,
            lane,
            depends_on,
            graph_revision,
        } => {
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.step_id == *step_id, "step_id")?;
            check(params.step_type == *step_type, "step_type")?;
            check(params.step_sequence == *step_sequence, "step_sequence")?;
            check(params.lane == *lane, "lane")?;
            check(params.depends_on == *depends_on, "depends_on")?;
            check(params.graph_revision == *graph_revision, "graph_revision")?;
            Ok(())
        }
        other => Err(wrong_variant("step_enqueued", other)),
    }
}

/// Validate that an [`AckParams`]'s envelope carries a `StepCompleted`
/// payload whose overlapping fields match the params.
pub(crate) fn validate_ack(params: &AckParams) -> Result<(), StoreError> {
    validate_routing(&params.completion_event)?;
    match &params.completion_event.payload {
        DomainEvent::StepCompleted {
            dispatch_id,
            step_id,
            step_type,
            result_payload,
            ..
        } => {
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.step_id == *step_id, "step_id")?;
            check(params.step_type == *step_type, "step_type")?;
            check(params.result == **result_payload, "result_payload")?;
            Ok(())
        }
        other => Err(wrong_variant("step_completed", other)),
    }
}

/// Validate that an [`AckAndEnqueueParams`]'s completion envelope
/// matches the completion fields, and that the optional next-step
/// envelope matches its enqueue params.
pub(crate) fn validate_ack_and_enqueue(params: &AckAndEnqueueParams) -> Result<(), StoreError> {
    validate_routing(&params.completion_event)?;
    match &params.completion_event.payload {
        DomainEvent::StepCompleted {
            dispatch_id,
            step_id,
            step_type,
            result_payload,
            ..
        } => {
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.step_id == *step_id, "step_id")?;
            check(params.step_type == *step_type, "step_type")?;
            check(params.result == **result_payload, "result_payload")?;
        }
        other => return Err(wrong_variant("step_completed", other)),
    }
    if let Some(ref next) = params.next_step {
        validate_enqueue_step(next)?;
        check(
            params.dispatch_id == next.dispatch_id,
            "next_step.dispatch_id must match params.dispatch_id",
        )?;
    }
    Ok(())
}

/// Validate that a [`NackParams`]'s envelope carries a `StepFailed`
/// payload whose overlapping fields match the params.
pub(crate) fn validate_nack(params: &NackParams) -> Result<(), StoreError> {
    validate_routing(&params.failure_event)?;
    match &params.failure_event.payload {
        DomainEvent::StepFailed {
            dispatch_id,
            step_id,
            step_type,
            error,
            error_class,
            ..
        } => {
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.step_id == *step_id, "step_id")?;
            check(params.step_type == *step_type, "step_type")?;
            check(params.error == *error, "error")?;
            check(params.error_class == *error_class, "error_class")?;
            Ok(())
        }
        other => Err(wrong_variant("step_failed", other)),
    }
}

/// Validate that a [`CreateDispatchParams`]'s envelope carries a
/// `DispatchCreated` payload whose fields match the params.
pub(crate) fn validate_create_dispatch(params: &CreateDispatchParams) -> Result<(), StoreError> {
    validate_routing(&params.creation_event)?;
    match &params.creation_event.payload {
        DomainEvent::DispatchCreated {
            dispatch_id,
            dispatch,
            mode,
            lane,
            actor,
            graph_revision,
        } => {
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.dispatch == **dispatch, "dispatch")?;
            check(params.mode == *mode, "mode")?;
            check(params.lane == *lane, "lane")?;
            check(params.actor == *actor, "actor")?;
            check(params.graph_revision == *graph_revision, "graph_revision")?;
            Ok(())
        }
        other => Err(wrong_variant("dispatch_created", other)),
    }
}

/// Validate that an [`UpdateDispatchStatusParams`]'s envelope carries
/// the lifecycle event matching the target status, with a consistent
/// outcome where applicable.
pub(crate) fn validate_update_dispatch_status(
    params: &UpdateDispatchStatusParams,
) -> Result<(), StoreError> {
    let expected_tag = dispatch_status_event_tag(params.status)?;
    validate_routing(&params.status_event)?;
    match &params.status_event.payload {
        DomainEvent::DispatchStarted { dispatch_id } => {
            check(expected_tag == "dispatch_started", "event_type")?;
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.outcome.is_none(), "outcome")?;
            Ok(())
        }
        DomainEvent::DispatchCompleted {
            dispatch_id,
            outcome,
            ..
        } => {
            check(expected_tag == "dispatch_completed", "event_type")?;
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.outcome == Some(*outcome), "outcome")?;
            Ok(())
        }
        DomainEvent::DispatchFailed {
            dispatch_id,
            outcome,
            ..
        } => {
            check(expected_tag == "dispatch_failed", "event_type")?;
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.outcome == Some(*outcome), "outcome")?;
            Ok(())
        }
        DomainEvent::DispatchCancelled { dispatch_id, .. } => {
            check(expected_tag == "dispatch_cancelled", "event_type")?;
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.outcome.is_none(), "outcome")?;
            Ok(())
        }
        other => Err(wrong_variant(expected_tag, other)),
    }
}

// ---- helpers ---------------------------------------------------------------

fn check(ok: bool, field: &str) -> Result<(), StoreError> {
    if ok {
        Ok(())
    } else {
        Err(StoreError::Conversion {
            context: "envelope validation",
            reason: format!("{field} mismatch between params and event payload"),
        })
    }
}

fn wrong_variant(expected: &str, actual: &DomainEvent) -> StoreError {
    let actual_tag = serde_json::to_value(actual)
        .ok()
        .and_then(|v| {
            v.get("event_type")
                .and_then(|t| t.as_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "<unknown>".to_owned());
    StoreError::Conversion {
        context: "envelope validation",
        reason: format!("expected event_type `{expected}`, got `{actual_tag}`"),
    }
}
