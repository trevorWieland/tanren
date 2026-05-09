//! Deployment-posture handlers shared across interfaces.
//!
//! This module owns posture listing, selection, and readback behavior. It keeps
//! transport parsing out of scope and returns typed contract failures for
//! policy/validation rejects.

use tanren_contract::{
    CurrentDeploymentPostureResponse, DeploymentPosture, DeploymentPostureCapabilitySummary,
    DeploymentPostureContractFailure, DeploymentPostureFailureReason, DeploymentPostureReadModel,
    DeploymentPostureScope, SetDeploymentPostureRequest, SetDeploymentPostureResponse,
    SupportedDeploymentPosture, SupportedDeploymentPosturesResponse,
};
use tanren_identity_policy::AccountId;
use tanren_policy::{
    Decision, DeploymentPosturePolicyInput, DeploymentPosturePolicyScope,
    evaluate_deployment_posture_management,
};
use tanren_store::{
    DeploymentPostureStore, NewDeploymentPosture, ResolvedDeploymentPostureScope, StoreError,
};
use thiserror::Error;

use crate::Clock;
use crate::events::{
    DEPLOYMENT_POSTURE_CHANGED_KIND, DeploymentPostureChanged, deployment_posture_envelope,
};

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

/// Return the shared permission/auth failure used when an interface
/// cannot resolve an authenticated actor from session context.
#[must_use]
pub fn missing_or_expired_session_failure() -> SetDeploymentPostureError {
    SetDeploymentPostureError::Contract {
        failure: DeploymentPostureContractFailure {
            reason: DeploymentPostureFailureReason::PermissionDenied,
            detail: DeploymentPostureFailureReason::PermissionDenied
                .summary()
                .to_owned(),
        },
    }
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

/// Return every supported posture with its capability explanation.
#[must_use]
pub fn list_supported_deployment_postures() -> SupportedDeploymentPosturesResponse {
    SupportedDeploymentPosturesResponse {
        supported: DeploymentPosture::ALL
            .into_iter()
            .map(|posture| SupportedDeploymentPosture {
                posture,
                capability_summary: DeploymentPostureCapabilitySummary::for_posture(posture),
            })
            .collect(),
    }
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
    S: DeploymentPostureStore + ?Sized,
{
    let posture = request.posture;
    let scope = request.scope;

    let resolved_scope = store
        .resolve_deployment_posture_scope(scope_to_store(scope))
        .await?
        .ok_or_else(|| SetDeploymentPostureError::Contract {
            failure: scope_not_found(scope),
        })?;

    if let Decision::Deny(_) =
        evaluate_deployment_posture_management(DeploymentPosturePolicyInput {
            actor,
            scope: policy_scope_from_resolved(resolved_scope),
        })
    {
        return Err(SetDeploymentPostureError::Contract {
            failure: permission_denied(actor, scope_from_store(resolved_scope.as_scope())),
        });
    }

    let now = clock.now();
    let store_scope = resolved_scope.as_scope();
    let scope = scope_from_store(store_scope);
    let changed_event = deployment_posture_envelope(
        DEPLOYMENT_POSTURE_CHANGED_KIND,
        &DeploymentPostureChanged {
            scope,
            posture,
            changed_by: actor,
            changed_at: now,
        },
    );
    let stored = store
        .upsert_deployment_posture_with_event(
            NewDeploymentPosture {
                scope: store_scope,
                posture: posture_to_store(posture),
                changed_by: actor,
                changed_at: now,
            },
            changed_event,
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
) -> Result<CurrentDeploymentPostureResponse, StoreError>
where
    S: DeploymentPostureStore + ?Sized,
{
    let row = store.get_deployment_posture(scope_to_store(scope)).await?;
    Ok(CurrentDeploymentPostureResponse {
        current: row.map(|stored| {
            to_read_model(
                scope_from_store(stored.scope),
                posture_from_store(stored.posture),
            )
        }),
    })
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

fn scope_not_found(scope: DeploymentPostureScope) -> DeploymentPostureContractFailure {
    DeploymentPostureContractFailure {
        reason: DeploymentPostureFailureReason::ScopeNotFound,
        detail: format!("Deployment posture scope {scope:?} does not exist."),
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

fn to_read_model(
    scope: DeploymentPostureScope,
    posture: DeploymentPosture,
) -> DeploymentPostureReadModel {
    DeploymentPostureReadModel {
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

const fn policy_scope_from_resolved(
    scope: ResolvedDeploymentPostureScope,
) -> DeploymentPosturePolicyScope {
    match scope {
        ResolvedDeploymentPostureScope::Account { account_id } => {
            DeploymentPosturePolicyScope::Account { account_id }
        }
        ResolvedDeploymentPostureScope::Project { project_id } => {
            DeploymentPosturePolicyScope::Project { project_id }
        }
        ResolvedDeploymentPostureScope::Installation { installation_id } => {
            DeploymentPosturePolicyScope::Installation { installation_id }
        }
    }
}
