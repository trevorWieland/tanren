//! Validation helper for atomic dispatch cancellation params.

use tanren_domain::DomainEvent;

use crate::errors::StoreError;
use crate::params::{CancelDispatchParams, UpdateDispatchStatusParams};

pub(crate) fn validate_cancel_dispatch(params: &CancelDispatchParams) -> Result<(), StoreError> {
    super::validate_update_dispatch_status(&UpdateDispatchStatusParams {
        dispatch_id: params.dispatch_id,
        status: tanren_domain::DispatchStatus::Cancelled,
        outcome: None,
        status_event: params.status_event.clone(),
    })?;
    match &params.status_event.payload {
        DomainEvent::DispatchCancelled {
            dispatch_id,
            actor,
            reason,
        } => {
            check(params.dispatch_id == *dispatch_id, "dispatch_id")?;
            check(params.actor == *actor, "actor")?;
            check(params.reason == *reason, "reason")?;
            Ok(())
        }
        other => Err(super::wrong_variant("dispatch_cancelled", other)),
    }
}

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
