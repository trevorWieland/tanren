//! Error mapping from orchestrator/store errors to contract errors.
//!
//! The contract crate cannot depend on the store crate (dependency DAG
//! rule: `contract → domain` only). This module bridges the gap by
//! mapping `OrchestratorError` (which wraps `StoreError` and
//! `DomainError`) to `ContractError`.

use tanren_contract::ContractError;
use tanren_orchestrator::OrchestratorError;
use tanren_store::StoreError;

/// Map an orchestrator error to a contract error.
pub fn map_orchestrator_error(err: OrchestratorError) -> ContractError {
    match err {
        OrchestratorError::Domain(domain_err) => ContractError::from(domain_err),
        OrchestratorError::Store(ref store_err) => map_store_error(store_err),
        OrchestratorError::PolicyDenied { decision } => ContractError::PolicyDenied {
            reason: decision
                .reason
                .unwrap_or_else(|| "policy denied".to_owned()),
        },
    }
}

/// Map a store error to a contract error.
fn map_store_error(err: &StoreError) -> ContractError {
    match err {
        StoreError::NotFound { entity } => ContractError::NotFound {
            entity: "entity".to_owned(),
            id: entity.clone(),
        },
        StoreError::InvalidTransition { entity, from, to } => ContractError::Conflict {
            reason: format!("invalid transition on {entity}: {from} -> {to}"),
        },
        StoreError::Conflict(reason) => ContractError::Conflict {
            reason: reason.clone(),
        },
        StoreError::Database(_)
        | StoreError::Migration(_)
        | StoreError::Conversion { .. }
        | StoreError::Json(_) => ContractError::Internal {
            message: err.to_string(),
        },
    }
}
