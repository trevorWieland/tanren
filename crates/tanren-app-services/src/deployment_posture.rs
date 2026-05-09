//! Deployment-posture handlers shared across interfaces.
//!
//! This module owns posture listing, selection, and readback behavior. It keeps
//! transport parsing out of scope and returns typed contract failures for
//! policy/validation rejects.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tanren_contract::{
    DeploymentPosture, DeploymentPostureCapabilitySummary, DeploymentPostureContractFailure,
    DeploymentPostureFailureReason, DeploymentPostureScope, SetDeploymentPostureRequest,
    SetDeploymentPostureResponse,
};
use tanren_identity_policy::AccountId;
use tanren_policy::{Decision, evaluate_account_scope_posture_management};
use tanren_store::{AccountStore, DeploymentPostureStore, NewDeploymentPosture, StoreError};
use thiserror::Error;

use crate::Clock;
use crate::events::{
    DEPLOYMENT_POSTURE_CHANGED_KIND, DeploymentPostureChanged, deployment_posture_envelope,
};

/// Supported posture option surfaced by `list_supported_deployment_postures`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportedDeploymentPosture {
    /// Canonical posture value.
    pub posture: DeploymentPosture,
    /// Canonical capability explanation for this posture.
    pub capability_summary: DeploymentPostureCapabilitySummary,
}

/// Error taxonomy for setting deployment posture at app-service scope.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SetDeploymentPostureError {
    /// The request failed contract-level validation or authorization.
    #[error("{detail}", detail = .failure.detail)]
    Contract {
        /// Contract failure payload.
        failure: DeploymentPostureContractFailure,
    },
    /// The underlying store layer raised an unexpected error.
    #[error(transparent)]
    Store {
        /// Wrapped persistence error.
        #[from]
        source: StoreError,
    },
}

impl SetDeploymentPostureError {
    /// Access the contract failure payload for transport mapping.
    #[must_use]
    pub fn contract_failure(&self) -> Option<&DeploymentPostureContractFailure> {
        if let Self::Contract { failure } = self {
            Some(failure)
        } else {
            None
        }
    }
}

const SUPPORTED_POSTURES: [DeploymentPosture; 3] = [
    DeploymentPosture::Hosted,
    DeploymentPosture::SelfHosted,
    DeploymentPosture::LocalOnly,
];

/// Return every supported posture with its capability explanation.
#[must_use]
pub fn list_supported_deployment_postures() -> Vec<SupportedDeploymentPosture> {
    SUPPORTED_POSTURES
        .into_iter()
        .map(|posture| SupportedDeploymentPosture {
            posture,
            capability_summary: DeploymentPostureCapabilitySummary::for_posture(posture),
        })
        .collect()
}

/// Persist a posture selection after validating the posture value and actor
/// permission model.
///
/// Current authorization model: account owners can only manage their own
/// account-scope posture. Project and installation scopes are denied until
/// organization/project grants land.
pub async fn set_deployment_posture<S>(
    store: &S,
    clock: &Clock,
    actor: AccountId,
    request: SetDeploymentPostureRequest,
) -> Result<SetDeploymentPostureResponse, SetDeploymentPostureError>
where
    S: DeploymentPostureStore + AccountStore + ?Sized,
{
    let posture = request
        .posture()
        .map_err(|failure| SetDeploymentPostureError::Contract { failure })?;
    let scope = request.scope;

    if let Decision::Deny(_) = evaluate_account_scope_posture_management(actor, scope) {
        return Err(SetDeploymentPostureError::Contract {
            failure: permission_denied(actor, scope),
        });
    }

    let now = clock.now();
    let stored = store
        .upsert_deployment_posture(NewDeploymentPosture {
            scope: scope_to_store(scope),
            posture: posture_to_store(posture),
            changed_by: actor,
            changed_at: now,
        })
        .await?;

    emit_posture_changed_event(
        store,
        scope_from_store(stored.scope),
        posture_from_store(stored.posture),
        stored.changed_by,
        stored.changed_at,
    )
    .await?;

    Ok(to_response(
        scope_from_store(stored.scope),
        posture_from_store(stored.posture),
    ))
}

/// Read the currently recorded posture view for a scope, if present.
pub async fn deployment_posture<S>(
    store: &S,
    scope: DeploymentPostureScope,
) -> Result<Option<SetDeploymentPostureResponse>, StoreError>
where
    S: DeploymentPostureStore + ?Sized,
{
    let row = store.get_deployment_posture(scope_to_store(scope)).await?;
    Ok(row.map(|stored| {
        to_response(
            scope_from_store(stored.scope),
            posture_from_store(stored.posture),
        )
    }))
}

async fn emit_posture_changed_event<S>(
    store: &S,
    scope: DeploymentPostureScope,
    posture: DeploymentPosture,
    changed_by: AccountId,
    changed_at: DateTime<Utc>,
) -> Result<(), StoreError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            deployment_posture_envelope(
                DEPLOYMENT_POSTURE_CHANGED_KIND,
                &DeploymentPostureChanged {
                    scope,
                    posture,
                    changed_by,
                    changed_at,
                },
            ),
            changed_at,
        )
        .await?;
    Ok(())
}

fn permission_denied(
    actor: AccountId,
    scope: DeploymentPostureScope,
) -> DeploymentPostureContractFailure {
    DeploymentPostureContractFailure {
        reason: DeploymentPostureFailureReason::PermissionDenied,
        detail: format!(
            "Actor {actor} may only change their own account-scope deployment posture; requested scope was {scope:?}."
        ),
    }
}

fn to_response(
    scope: DeploymentPostureScope,
    posture: DeploymentPosture,
) -> SetDeploymentPostureResponse {
    SetDeploymentPostureResponse {
        scope,
        posture,
        capability_summary: DeploymentPostureCapabilitySummary::for_posture(posture),
    }
}

const fn scope_to_store(scope: DeploymentPostureScope) -> tanren_store::DeploymentPostureScope {
    match scope {
        DeploymentPostureScope::Account { account_id } => {
            tanren_store::DeploymentPostureScope::Account { account_id }
        }
        DeploymentPostureScope::Project { project_id } => {
            tanren_store::DeploymentPostureScope::Project { project_id }
        }
        DeploymentPostureScope::Installation { installation_id } => {
            tanren_store::DeploymentPostureScope::Installation { installation_id }
        }
    }
}

const fn scope_from_store(scope: tanren_store::DeploymentPostureScope) -> DeploymentPostureScope {
    match scope {
        tanren_store::DeploymentPostureScope::Account { account_id } => {
            DeploymentPostureScope::Account { account_id }
        }
        tanren_store::DeploymentPostureScope::Project { project_id } => {
            DeploymentPostureScope::Project { project_id }
        }
        tanren_store::DeploymentPostureScope::Installation { installation_id } => {
            DeploymentPostureScope::Installation { installation_id }
        }
    }
}

const fn posture_to_store(posture: DeploymentPosture) -> tanren_store::DeploymentPosture {
    match posture {
        DeploymentPosture::Hosted => tanren_store::DeploymentPosture::Hosted,
        DeploymentPosture::SelfHosted => tanren_store::DeploymentPosture::SelfHosted,
        DeploymentPosture::LocalOnly => tanren_store::DeploymentPosture::LocalOnly,
    }
}

const fn posture_from_store(posture: tanren_store::DeploymentPosture) -> DeploymentPosture {
    match posture {
        tanren_store::DeploymentPosture::Hosted => DeploymentPosture::Hosted,
        tanren_store::DeploymentPosture::SelfHosted => DeploymentPosture::SelfHosted,
        tanren_store::DeploymentPosture::LocalOnly => DeploymentPosture::LocalOnly,
    }
}
