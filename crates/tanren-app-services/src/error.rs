//! Error mapping from orchestrator/store errors to contract errors.
//!
//! The contract crate cannot depend on the store crate (dependency DAG
//! rule: `contract → domain` only). This module bridges the gap by
//! mapping `OrchestratorError` (which wraps `StoreError` and
//! `DomainError`) to `ContractError`.

use tanren_contract::ContractError;
use tanren_observability::{
    ObservabilityError, emit_and_build_internal_error_response,
    emit_and_build_internal_error_response_with_emitter,
};
use tanren_orchestrator::OrchestratorError;
use tanren_store::{StoreConflictClass, StoreError};

use crate::auth::AuthFailureKind;

type EmitCorrelatedInternalError =
    fn(&str, &str, uuid::Uuid, &str) -> Result<(), ObservabilityError>;

/// Map an orchestrator error to a wire-safe contract error response.
pub fn map_orchestrator_error(err: OrchestratorError) -> tanren_contract::ErrorResponse {
    match err {
        OrchestratorError::Domain(domain_err) => {
            tanren_contract::ErrorResponse::from(ContractError::from(domain_err))
        }
        OrchestratorError::Store(ref store_err) => map_store_error(store_err),
        OrchestratorError::PolicyDenied { decision } => {
            tanren_contract::ErrorResponse::from(ContractError::PolicyDenied {
                reason_code: decision.reason_code,
            })
        }
    }
}

/// Map a store error to a wire-safe contract error response.
pub fn map_store_error(err: &StoreError) -> tanren_contract::ErrorResponse {
    match err {
        StoreError::NotFound { entity_kind, id } => {
            tanren_contract::ErrorResponse::from(ContractError::NotFound {
                entity: entity_kind.to_string(),
                id: id.clone(),
            })
        }
        StoreError::InvalidTransition { entity, from, to } => {
            tanren_contract::ErrorResponse::from(ContractError::InvalidTransition {
                entity: entity.clone(),
                from: from.clone(),
                to: to.clone(),
            })
        }
        StoreError::Conflict {
            class,
            operation,
            reason,
        } => match class {
            StoreConflictClass::Contention => {
                tanren_contract::ErrorResponse::from(ContractError::ContentionConflict {
                    operation: operation.to_string(),
                    reason: reason.clone(),
                })
            }
            StoreConflictClass::Other => {
                tanren_contract::ErrorResponse::from(ContractError::Conflict {
                    reason: reason.clone(),
                })
            }
        },
        StoreError::SchemaNotReady { reason } => {
            tanren_contract::ErrorResponse::from(ContractError::SchemaNotReady {
                reason: reason.clone(),
            })
        }
        StoreError::ReplayRejected => auth_failure_response(AuthFailureKind::ReplayRejected),
        StoreError::Database(_)
        | StoreError::Migration(_)
        | StoreError::Conversion { .. }
        | StoreError::Json(_) => {
            correlated_internal_error_response("tanren_app_services", "internal", &err.to_string())
        }
    }
}

fn auth_failure_response(kind: AuthFailureKind) -> tanren_contract::ErrorResponse {
    auth_failure_response_with_emitter(kind, tanren_observability::emit_correlated_internal_error)
}

fn auth_failure_response_with_emitter(
    kind: AuthFailureKind,
    emitter: EmitCorrelatedInternalError,
) -> tanren_contract::ErrorResponse {
    match kind {
        AuthFailureKind::InvalidToken | AuthFailureKind::ReplayRejected => {
            tanren_contract::ErrorResponse::from(ContractError::InvalidField {
                field: "actor_token".to_owned(),
                reason: "token validation failed".to_owned(),
            })
        }
        AuthFailureKind::BackendFailure => emit_and_build_internal_error_response_with_emitter(
            "tanren_app_services",
            "auth_backend_failure",
            "auth backend failure",
            emitter,
        ),
    }
}

fn correlated_internal_error_response(
    component: &str,
    error_code: &str,
    raw_error: &str,
) -> tanren_contract::ErrorResponse {
    emit_and_build_internal_error_response(component, error_code, raw_error)
}
