//! Validation helper for atomic dispatch creation with initial step.

use crate::errors::StoreError;
use crate::params::CreateDispatchWithInitialStepParams;

pub(crate) fn validate_create_dispatch_with_initial_step(
    params: &CreateDispatchWithInitialStepParams,
) -> Result<(), StoreError> {
    super::validate_create_dispatch(&params.dispatch)?;
    super::validate_enqueue_step(&params.initial_step)?;

    check(
        params.dispatch.dispatch_id == params.initial_step.dispatch_id,
        "dispatch_id",
    )?;
    check(
        params.initial_step.step_type == tanren_domain::StepType::Provision,
        "initial_step.step_type",
    )?;
    check(
        params.initial_step.step_sequence == 0,
        "initial_step.step_sequence",
    )?;
    check(
        params.initial_step.depends_on.is_empty(),
        "initial_step.depends_on",
    )?;
    check(
        params.initial_step.ready_state == tanren_domain::StepReadyState::Ready,
        "initial_step.ready_state",
    )?;
    Ok(())
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
