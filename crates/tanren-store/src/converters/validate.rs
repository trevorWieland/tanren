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

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tanren_domain::{
        ActorContext, ConfigKeys, DispatchId, DispatchMode, DispatchSnapshot, DispatchStatus,
        ErrorClass, EventEnvelope, EventId, FiniteF64, GraphRevision, Lane, NonEmptyString,
        Outcome, Phase, StepId, StepType, TimeoutSecs,
    };
    use uuid::Uuid;

    use super::*;
    use crate::params::{AckParams, EnqueueStepParams, NackParams};

    fn snap() -> DispatchSnapshot {
        DispatchSnapshot {
            project: NonEmptyString::try_new("p".to_owned()).expect("p"),
            phase: Phase::DoTask,
            cli: tanren_domain::Cli::Claude,
            auth_mode: tanren_domain::AuthMode::ApiKey,
            branch: NonEmptyString::try_new("main".to_owned()).expect("b"),
            spec_folder: NonEmptyString::try_new("spec".to_owned()).expect("s"),
            workflow_id: NonEmptyString::try_new("wf".to_owned()).expect("w"),
            timeout: TimeoutSecs::try_new(60).expect("t"),
            environment_profile: NonEmptyString::try_new("d".to_owned()).expect("e"),
            gate_cmd: None,
            context: None,
            model: None,
            project_env: ConfigKeys::default(),
            required_secrets: vec![],
            preserve_on_failure: false,
            created_at: Utc::now(),
        }
    }

    fn actor() -> ActorContext {
        ActorContext {
            org_id: tanren_domain::OrgId::new(),
            user_id: tanren_domain::UserId::new(),
            team_id: None,
            api_key_id: None,
            project_id: None,
        }
    }

    fn evt(payload: DomainEvent) -> EventEnvelope {
        EventEnvelope::new(EventId::from_uuid(Uuid::now_v7()), Utc::now(), payload)
    }

    fn test_result() -> tanren_domain::StepResult {
        tanren_domain::StepResult::Provision(Box::new(tanren_domain::ProvisionResult {
            handle: tanren_domain::EnvironmentHandle {
                id: NonEmptyString::try_new("h".to_owned()).expect("h"),
                runtime_type: NonEmptyString::try_new("local".to_owned()).expect("rt"),
            },
        }))
    }

    // ---- enqueue_step ---------------------------------------------------

    #[test]
    fn enqueue_step_accepts_matching_fields() {
        let did = DispatchId::new();
        let sid = StepId::new();
        let params = EnqueueStepParams {
            dispatch_id: did,
            step_id: sid,
            step_type: StepType::Execute,
            step_sequence: 0,
            lane: Some(Lane::Impl),
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
            payload: tanren_domain::StepPayload::Provision(Box::new(
                tanren_domain::ProvisionPayload { dispatch: snap() },
            )),
            ready_state: tanren_domain::StepReadyState::Ready,
            enqueue_event: evt(DomainEvent::StepEnqueued {
                dispatch_id: did,
                step_id: sid,
                step_type: StepType::Execute,
                step_sequence: 0,
                lane: Some(Lane::Impl),
                depends_on: vec![],
                graph_revision: GraphRevision::INITIAL,
            }),
        };
        assert!(validate_enqueue_step(&params).is_ok());
    }

    #[test]
    fn enqueue_step_rejects_step_type_mismatch() {
        let did = DispatchId::new();
        let sid = StepId::new();
        let params = EnqueueStepParams {
            dispatch_id: did,
            step_id: sid,
            step_type: StepType::Execute,
            step_sequence: 0,
            lane: None,
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
            payload: tanren_domain::StepPayload::Provision(Box::new(
                tanren_domain::ProvisionPayload { dispatch: snap() },
            )),
            ready_state: tanren_domain::StepReadyState::Ready,
            enqueue_event: evt(DomainEvent::StepEnqueued {
                dispatch_id: did,
                step_id: sid,
                step_type: StepType::Provision, // mismatch
                step_sequence: 0,
                lane: None,
                depends_on: vec![],
                graph_revision: GraphRevision::INITIAL,
            }),
        };
        let err = validate_enqueue_step(&params).expect_err("should reject");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    #[test]
    fn enqueue_step_rejects_wrong_variant() {
        let did = DispatchId::new();
        let sid = StepId::new();
        let params = EnqueueStepParams {
            dispatch_id: did,
            step_id: sid,
            step_type: StepType::Execute,
            step_sequence: 0,
            lane: None,
            depends_on: vec![],
            graph_revision: GraphRevision::INITIAL,
            payload: tanren_domain::StepPayload::Provision(Box::new(
                tanren_domain::ProvisionPayload { dispatch: snap() },
            )),
            ready_state: tanren_domain::StepReadyState::Ready,
            enqueue_event: evt(DomainEvent::StepCompleted {
                dispatch_id: did,
                step_id: sid,
                step_type: StepType::Execute,
                duration_secs: FiniteF64::try_new(1.0).expect("f"),
                result_payload: Box::new(test_result()),
            }),
        };
        let err = validate_enqueue_step(&params).expect_err("should reject");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    // ---- ack ------------------------------------------------------------

    #[test]
    fn ack_rejects_result_mismatch() {
        let did = DispatchId::new();
        let sid = StepId::new();
        let result_a = test_result();
        // Different handle id => different result
        let result_b =
            tanren_domain::StepResult::Provision(Box::new(tanren_domain::ProvisionResult {
                handle: tanren_domain::EnvironmentHandle {
                    id: NonEmptyString::try_new("other".to_owned()).expect("o"),
                    runtime_type: NonEmptyString::try_new("local".to_owned()).expect("rt"),
                },
            }));
        let params = AckParams {
            dispatch_id: did,
            step_id: sid,
            step_type: StepType::Provision,
            result: result_a,
            completion_event: evt(DomainEvent::StepCompleted {
                dispatch_id: did,
                step_id: sid,
                step_type: StepType::Provision,
                duration_secs: FiniteF64::try_new(1.0).expect("f"),
                result_payload: Box::new(result_b),
            }),
        };
        let err = validate_ack(&params).expect_err("should reject");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    // ---- nack -----------------------------------------------------------

    #[test]
    fn nack_rejects_error_class_mismatch() {
        let did = DispatchId::new();
        let sid = StepId::new();
        let params = NackParams {
            dispatch_id: did,
            step_id: sid,
            step_type: StepType::Execute,
            error: "boom".to_owned(),
            error_class: ErrorClass::Transient,
            retry: false,
            failure_event: evt(DomainEvent::StepFailed {
                dispatch_id: did,
                step_id: sid,
                step_type: StepType::Execute,
                error: "boom".to_owned(),
                error_class: ErrorClass::Fatal, // mismatch
                retry_count: 0,
                duration_secs: FiniteF64::try_new(1.0).expect("f"),
            }),
        };
        let err = validate_nack(&params).expect_err("should reject");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    // ---- create_dispatch ------------------------------------------------

    #[test]
    fn create_dispatch_rejects_mode_mismatch() {
        let did = DispatchId::new();
        let s = snap();
        let a = actor();
        let params = CreateDispatchParams {
            dispatch_id: did,
            mode: DispatchMode::Manual,
            lane: Lane::Impl,
            dispatch: s.clone(),
            actor: a.clone(),
            graph_revision: GraphRevision::INITIAL,
            created_at: Utc::now(),
            creation_event: evt(DomainEvent::DispatchCreated {
                dispatch_id: did,
                dispatch: Box::new(s),
                mode: DispatchMode::Auto, // mismatch
                lane: Lane::Impl,
                actor: a,
                graph_revision: GraphRevision::INITIAL,
            }),
        };
        let err = validate_create_dispatch(&params).expect_err("should reject");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }

    // ---- update_dispatch_status -----------------------------------------

    #[test]
    fn update_dispatch_status_rejects_pending() {
        let did = DispatchId::new();
        let params = UpdateDispatchStatusParams {
            dispatch_id: did,
            status: DispatchStatus::Pending,
            outcome: None,
            status_event: evt(DomainEvent::DispatchStarted { dispatch_id: did }),
        };
        let err = validate_update_dispatch_status(&params).expect_err("should reject");
        assert!(matches!(err, StoreError::InvalidTransition { .. }));
    }

    #[test]
    fn update_dispatch_status_rejects_outcome_mismatch() {
        let did = DispatchId::new();
        let params = UpdateDispatchStatusParams {
            dispatch_id: did,
            status: DispatchStatus::Completed,
            outcome: Some(Outcome::Fail),
            status_event: evt(DomainEvent::DispatchCompleted {
                dispatch_id: did,
                outcome: Outcome::Success, // mismatch
                total_duration_secs: FiniteF64::try_new(1.0).expect("f"),
            }),
        };
        let err = validate_update_dispatch_status(&params).expect_err("should reject");
        assert!(matches!(err, StoreError::Conversion { .. }));
    }
}
