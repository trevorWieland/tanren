//! Error mapping from orchestrator/store errors to contract errors.
//!
//! The contract crate cannot depend on the store crate (dependency DAG
//! rule: `contract → domain` only). This module bridges the gap by
//! mapping `OrchestratorError` (which wraps `StoreError` and
//! `DomainError`) to `ContractError`.

use tanren_contract::ContractError;
use tanren_observability::{ObservabilityError, emit_correlated_internal_error};
use tanren_orchestrator::OrchestratorError;
use tanren_store::{StoreConflictClass, StoreError};
use uuid::Uuid;

type EmitCorrelatedInternalError = fn(&str, &str, Uuid, &str) -> Result<(), ObservabilityError>;

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
        StoreError::Database(_)
        | StoreError::Migration(_)
        | StoreError::Conversion { .. }
        | StoreError::Json(_) => {
            correlated_internal_error_response("tanren_app_services", "internal", &err.to_string())
        }
    }
}

fn correlated_internal_error_response(
    component: &str,
    error_code: &str,
    raw_error: &str,
) -> tanren_contract::ErrorResponse {
    correlated_internal_error_response_with_emitter(
        component,
        error_code,
        raw_error,
        emit_correlated_internal_error,
    )
}

fn correlated_internal_error_response_with_emitter(
    component: &str,
    error_code: &str,
    raw_error: &str,
    emitter: EmitCorrelatedInternalError,
) -> tanren_contract::ErrorResponse {
    let correlation_id = Uuid::now_v7();
    let details = if emitter(component, error_code, correlation_id, raw_error).is_ok() {
        Some(serde_json::json!({
            "correlation_id": correlation_id.to_string(),
        }))
    } else {
        None
    };

    tanren_contract::ErrorResponse {
        code: tanren_contract::ErrorCode::Internal,
        message: "internal error".to_owned(),
        details,
    }
}

#[cfg(test)]
mod tests {
    use tanren_domain::{
        ActorContext, DomainError, EntityKind, EntityRef, OrgId, PolicyDecisionKind,
        PolicyDecisionRecord, PolicyOutcome, PolicyReasonCode, PolicyResourceRef, PolicyScope,
        UserId,
    };
    use tanren_orchestrator::OrchestratorError;
    use tanren_store::{StoreConflictClass, StoreError, StoreOperation};

    use super::correlated_internal_error_response_with_emitter;
    use super::map_orchestrator_error;

    #[test]
    fn store_not_found_maps_to_typed_contract_not_found() {
        let err = OrchestratorError::Store(StoreError::NotFound {
            entity_kind: EntityKind::Dispatch,
            id: "123".to_owned(),
        });
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, tanren_contract::ErrorCode::NotFound);
        assert!(mapped.message.contains("dispatch"));
        assert!(mapped.message.contains("123"));
    }

    #[test]
    fn correlated_internal_error_includes_correlation_id_when_sink_persists() {
        fn emitter(
            _component: &str,
            _error_code: &str,
            _correlation_id: uuid::Uuid,
            raw_error: &str,
        ) -> Result<(), tanren_observability::ObservabilityError> {
            if raw_error == "__force_sink_error__" {
                return Err(tanren_observability::ObservabilityError::SinkIo(
                    "disk full".to_owned(),
                ));
            }
            Ok(())
        }

        let mapped = correlated_internal_error_response_with_emitter(
            "tanren_app_services",
            "internal",
            "db specifics",
            emitter,
        );
        assert_eq!(mapped.code, tanren_contract::ErrorCode::Internal);
        assert_eq!(mapped.message, "internal error");
        let details = mapped.details.expect("details");
        let correlation_id = details
            .get("correlation_id")
            .and_then(serde_json::Value::as_str)
            .expect("correlation_id");
        assert!(uuid::Uuid::parse_str(correlation_id).is_ok());
    }

    #[test]
    fn correlated_internal_error_omits_correlation_id_when_sink_persist_fails() {
        fn emitter(
            _component: &str,
            _error_code: &str,
            _correlation_id: uuid::Uuid,
            raw_error: &str,
        ) -> Result<(), tanren_observability::ObservabilityError> {
            if raw_error == "__force_sink_error__" {
                return Err(tanren_observability::ObservabilityError::SinkIo(
                    "disk full".to_owned(),
                ));
            }
            Ok(())
        }

        let mapped = correlated_internal_error_response_with_emitter(
            "tanren_app_services",
            "internal",
            "__force_sink_error__",
            emitter,
        );
        assert_eq!(mapped.code, tanren_contract::ErrorCode::Internal);
        assert_eq!(mapped.message, "internal error");
        assert!(
            mapped.details.is_none(),
            "correlation_id must be omitted when sink persistence fails"
        );
    }

    #[test]
    fn domain_not_found_maps_without_internal_shape() {
        let err = OrchestratorError::Domain(DomainError::NotFound {
            entity: EntityRef::Dispatch(tanren_domain::DispatchId::new()),
        });
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, tanren_contract::ErrorCode::NotFound);
        assert!(
            mapped.message.starts_with("dispatch not found: "),
            "expected canonical not_found message: {}",
            mapped.message
        );
        assert!(!mapped.message.contains("dispatch dispatch"));
        assert!(mapped.details.is_none());
    }

    #[test]
    fn domain_policy_denied_maps_to_canonical_wire_shape() {
        let err = OrchestratorError::Domain(DomainError::PolicyDenied {
            kind: PolicyDecisionKind::Authz,
            resource: PolicyResourceRef::Dispatch {
                dispatch_id: tanren_domain::DispatchId::new(),
            },
            reason: "nope".to_owned(),
        });
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, tanren_contract::ErrorCode::PolicyDenied);
        assert_eq!(mapped.message, "policy denied");
        assert!(mapped.details.is_none());
    }

    #[test]
    fn policy_denied_exposes_only_reason_code_details() {
        let err = OrchestratorError::PolicyDenied {
            decision: Box::new(PolicyDecisionRecord {
                kind: PolicyDecisionKind::Authz,
                resource: PolicyResourceRef::Dispatch {
                    dispatch_id: tanren_domain::DispatchId::new(),
                },
                scope: PolicyScope::new(ActorContext::new(OrgId::new(), UserId::new())),
                outcome: PolicyOutcome::Denied,
                reason_code: Some(PolicyReasonCode::TimeoutOutOfRange),
                reason: Some("timeout too high".to_owned()),
            }),
        };
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, tanren_contract::ErrorCode::PolicyDenied);
        assert_eq!(mapped.message, "policy denied");
        let details = mapped.details.expect("details");
        assert_eq!(details["reason_code"], "timeout_out_of_range");
        let object = details.as_object().expect("object");
        assert_eq!(object.len(), 1, "only reason_code should be present");
        assert!(!object.contains_key("reason"));
        assert!(!object.contains_key("reason_message"));
        assert!(!object.contains_key("decision_kind"));
        assert!(!object.contains_key("resource"));
    }

    #[test]
    fn contention_conflict_maps_to_typed_wire_code() {
        let err = OrchestratorError::Store(StoreError::Conflict {
            class: StoreConflictClass::Contention,
            operation: StoreOperation::CancelDispatch,
            reason: "cancel contention".to_owned(),
        });
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, tanren_contract::ErrorCode::ContentionConflict);
    }

    #[test]
    fn invalid_transition_maps_to_typed_wire_code() {
        let err = OrchestratorError::Store(StoreError::InvalidTransition {
            entity: "dispatch abc".to_owned(),
            from: "running".to_owned(),
            to: "cancelled".to_owned(),
        });
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, tanren_contract::ErrorCode::InvalidTransition);
    }

    #[test]
    fn schema_not_ready_maps_to_typed_wire_code() {
        let err = OrchestratorError::Store(StoreError::SchemaNotReady {
            reason: "missing migration metadata table".to_owned(),
        });
        let mapped = map_orchestrator_error(err);
        assert_eq!(mapped.code, tanren_contract::ErrorCode::SchemaNotReady);
    }
}
