use tanren_contract::RoleFailureReason;

use super::api::{code_to_reason, role_code_to_reason};
use super::tui_driver::TuiTranscript;
use super::{HarnessError, RoleHarnessError};

pub(crate) fn parse_account_failure(transcript: &TuiTranscript) -> Option<HarnessError> {
    let known_codes = [
        "duplicate_identifier",
        "invalid_credential",
        "validation_failed",
        "invitation_not_found",
        "invitation_expired",
        "invitation_already_consumed",
    ];
    let code = known_codes
        .iter()
        .copied()
        .find(|candidate| transcript.contains(candidate))?;
    let reason = code_to_reason(code)?;
    Some(HarnessError::Account(reason, code.to_owned()))
}

pub(crate) fn parse_role_failure(transcript: &TuiTranscript) -> Option<RoleHarnessError> {
    let known_codes = [
        "validation_failed",
        "not_found",
        "conflict",
        "permission_denied",
        "role_as_principal_rejected",
        "internal_error",
    ];
    let code = known_codes
        .iter()
        .copied()
        .find(|candidate| transcript.contains(candidate))?;
    let reason = role_code_to_reason(code).unwrap_or(RoleFailureReason::InternalError);
    Some(RoleHarnessError::Role(reason, code.to_owned()))
}
