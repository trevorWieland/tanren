use tanren_app_services::AppServiceError;
use tanren_contract::AccountFailureReason;
use tanren_identity_policy::{AccountId, OrgId};

pub(crate) fn organization_route_span(
    operation: &'static str,
    org_id: Option<OrgId>,
) -> tracing::Span {
    let span = tracing::info_span!(
        target: "tanren_api",
        "organization_route",
        operation,
        authenticated_account_id = tracing::field::Empty,
        organization_id = tracing::field::Empty,
    );
    if let Some(org_id) = org_id {
        span.record("organization_id", tracing::field::display(org_id));
    }
    span
}

pub(crate) fn record_authenticated_account(span: &tracing::Span, account_id: AccountId) {
    span.record(
        "authenticated_account_id",
        tracing::field::display(account_id),
    );
}

pub(crate) fn emit_route_success(
    operation: &'static str,
    account_id: AccountId,
    org_id: Option<OrgId>,
) {
    tracing::info!(
        target: "tanren_api",
        operation,
        authenticated_account_id = %account_id,
        organization_id = ?org_id,
        outcome = "success",
        "organization route completed"
    );
}

pub(crate) fn emit_route_failure(
    operation: &'static str,
    account_id: AccountId,
    org_id: Option<OrgId>,
    err: &AppServiceError,
) {
    match err {
        AppServiceError::Account(reason) => {
            let reason: AccountFailureReason = *reason;
            tracing::warn!(
                target: "tanren_api",
                operation,
                authenticated_account_id = %account_id,
                organization_id = ?org_id,
                outcome = "denial",
                denial_code = reason.code(),
                "organization route denied"
            );
        }
        _ => {
            tracing::error!(
                target: "tanren_api",
                operation,
                authenticated_account_id = %account_id,
                organization_id = ?org_id,
                outcome = "error",
                error = %err,
                "organization route failed"
            );
        }
    }
}

pub(crate) fn emit_route_auth_denial(operation: &'static str, org_id: Option<OrgId>) {
    tracing::warn!(
        target: "tanren_api",
        operation,
        organization_id = ?org_id,
        outcome = "denial",
        denial_code = "auth_required",
        "organization route denied"
    );
}
