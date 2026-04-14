//! Error mapping from orchestrator/store errors to contract errors.
//!
//! The contract crate cannot depend on the store crate (dependency DAG
//! rule: `contract → domain` only). This module bridges the gap by
//! mapping `OrchestratorError` (which wraps `StoreError` and
//! `DomainError`) to `ContractError`.

use tanren_contract::ContractError;
use tanren_orchestrator::OrchestratorError;
use tanren_store::StoreError;
use tracing::error;
use uuid::Uuid;

/// Map an orchestrator error to a wire-safe contract error response.
pub fn map_orchestrator_error(err: OrchestratorError) -> tanren_contract::ErrorResponse {
    match err {
        OrchestratorError::Domain(domain_err) => {
            tanren_contract::ErrorResponse::from(ContractError::from(domain_err))
        }
        OrchestratorError::Store(ref store_err) => map_store_error(store_err),
        OrchestratorError::PolicyDenied { decision } => {
            tanren_contract::ErrorResponse::from(ContractError::PolicyDenied {
                reason: decision
                    .reason
                    .unwrap_or_else(|| "policy denied".to_owned()),
            })
        }
    }
}

/// Map a store error to a wire-safe contract error response.
fn map_store_error(err: &StoreError) -> tanren_contract::ErrorResponse {
    match err {
        StoreError::NotFound { entity_kind, id } => {
            tanren_contract::ErrorResponse::from(ContractError::NotFound {
                entity: entity_kind.to_string(),
                id: id.clone(),
            })
        }
        StoreError::InvalidTransition { entity, from, to } => {
            tanren_contract::ErrorResponse::from(ContractError::Conflict {
                reason: format!("invalid transition on {entity}: {from} -> {to}"),
            })
        }
        StoreError::Conflict(reason) => {
            tanren_contract::ErrorResponse::from(ContractError::Conflict {
                reason: reason.clone(),
            })
        }
        StoreError::Database(_)
        | StoreError::Migration(_)
        | StoreError::Conversion { .. }
        | StoreError::Json(_) => {
            let correlation_id = Uuid::now_v7();
            error!(
                %correlation_id,
                error = %err,
                ?err,
                "store internal failure mapped to contract internal error",
            );
            tanren_contract::ErrorResponse {
                code: "internal".to_owned(),
                message: "internal error".to_owned(),
                details: Some(serde_json::json!({
                    "correlation_id": correlation_id.to_string(),
                })),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tanren_domain::{DomainError, EntityKind, EntityRef};
    use tanren_orchestrator::OrchestratorError;
    use tanren_store::StoreError;

    use super::map_orchestrator_error;

    #[test]
    fn store_not_found_maps_to_typed_contract_not_found() {
        let err = OrchestratorError::Store(StoreError::NotFound {
            entity_kind: EntityKind::Dispatch,
            id: "123".to_owned(),
        });
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, "not_found");
        assert!(mapped.message.contains("dispatch"));
        assert!(mapped.message.contains("123"));
    }

    #[test]
    fn store_internal_is_sanitized_with_correlation_id() {
        let err = OrchestratorError::Store(StoreError::Migration("db specifics".to_owned()));
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, "internal");
        assert_eq!(mapped.message, "internal error");
        let details = mapped.details.expect("details");
        let correlation_id = details
            .get("correlation_id")
            .and_then(serde_json::Value::as_str)
            .expect("correlation_id");
        assert!(uuid::Uuid::parse_str(correlation_id).is_ok());
    }

    #[test]
    fn domain_not_found_maps_without_internal_shape() {
        let err = OrchestratorError::Domain(DomainError::NotFound {
            entity: EntityRef::Dispatch(tanren_domain::DispatchId::new()),
        });
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, "not_found");
        assert!(mapped.details.is_none());
    }
}
