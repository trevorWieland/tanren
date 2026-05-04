//! Posture-flow handlers: list, get, set deployment posture.
//!
//! Handlers are mechanism-neutral at the contract surface. The permission
//! model is minimal: an explicit [`Actor`] with a [`Permissions`] set;
//! surfaces translate their session into an `Actor` before calling the
//! handler.
//!
//! The store layer (`PostureStore`) owns persistence and optimistic
//! concurrency; this module owns permission verification, capability-
//! summary derivation, and event emission.

use chrono::{DateTime, Utc};
use tanren_contract::{
    GetPostureResponse, ListPosturesResponse, PostureChangeView, PostureFailureReason, PostureView,
    SetPostureResponse,
};
use tanren_domain::{CapabilityAvailability, CapabilityCategory, CapabilitySummary, Posture};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, PostureStore};

use crate::events::{PostureEventKind, PostureSet, PostureSetRejected, posture_envelope};
use crate::{AppServiceError, Clock};

/// Minimal permission set carried by the caller. Surfaces translate
/// their session into an `Actor` with the appropriate permissions.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    /// Whether the actor may change the deployment posture.
    pub posture_admin: bool,
}

/// Actor identity passed into every posture handler. Surfaces construct
/// this from the current session before invoking the handler.
#[derive(Debug, Clone)]
pub struct Actor {
    /// Account invoking the operation.
    pub account_id: AccountId,
    /// Permissions granted to the actor.
    pub permissions: Permissions,
}

fn capabilities_for_posture(posture: Posture) -> Vec<CapabilitySummary> {
    use CapabilityAvailability::{Available, Unavailable};
    use CapabilityCategory::*;

    match posture {
        Posture::Hosted => vec![
            CapabilitySummary {
                category: Compute,
                availability: Available,
            },
            CapabilitySummary {
                category: Storage,
                availability: Available,
            },
            CapabilitySummary {
                category: Networking,
                availability: Available,
            },
            CapabilitySummary {
                category: Collaboration,
                availability: Available,
            },
            CapabilitySummary {
                category: Secrets,
                availability: Available,
            },
            CapabilitySummary {
                category: ProviderIntegration,
                availability: Available,
            },
        ],
        Posture::SelfHosted => vec![
            CapabilitySummary {
                category: Compute,
                availability: Available,
            },
            CapabilitySummary {
                category: Storage,
                availability: Available,
            },
            CapabilitySummary {
                category: Networking,
                availability: Available,
            },
            CapabilitySummary {
                category: Collaboration,
                availability: Available,
            },
            CapabilitySummary {
                category: Secrets,
                availability: Available,
            },
            CapabilitySummary {
                category: ProviderIntegration,
                availability: Available,
            },
        ],
        Posture::LocalOnly => vec![
            CapabilitySummary {
                category: Compute,
                availability: Available,
            },
            CapabilitySummary {
                category: Storage,
                availability: Available,
            },
            CapabilitySummary {
                category: Networking,
                availability: Unavailable {
                    reason: "Local-only installations do not have external network connectivity."
                        .to_owned(),
                },
            },
            CapabilitySummary {
                category: Collaboration,
                availability: Unavailable {
                    reason: "Local-only installations do not support multi-user collaboration."
                        .to_owned(),
                },
            },
            CapabilitySummary {
                category: Secrets,
                availability: Available,
            },
            CapabilitySummary {
                category: ProviderIntegration,
                availability: Unavailable {
                    reason: "Local-only installations cannot connect to external providers."
                        .to_owned(),
                },
            },
        ],
    }
}

fn posture_view(posture: Posture) -> PostureView {
    PostureView {
        posture,
        capabilities: capabilities_for_posture(posture),
    }
}

pub(crate) fn list_postures() -> ListPosturesResponse {
    let postures = Posture::all().iter().map(|&p| posture_view(p)).collect();
    ListPosturesResponse { postures }
}

pub(crate) async fn get_posture<S>(store: &S) -> Result<GetPostureResponse, AppServiceError>
where
    S: PostureStore + ?Sized,
{
    let record = store.current_posture().await?;
    match record {
        Some(rec) => Ok(GetPostureResponse {
            current: posture_view(rec.posture),
        }),
        None => Err(AppServiceError::Posture(
            PostureFailureReason::NotConfigured,
        )),
    }
}

pub(crate) async fn set_posture<S>(
    store: &S,
    clock: &Clock,
    actor: &Actor,
    posture_str: &str,
) -> Result<SetPostureResponse, AppServiceError>
where
    S: PostureStore + AccountStore + ?Sized,
{
    let now = clock.now();

    if !actor.permissions.posture_admin {
        emit_rejected(
            store,
            actor.account_id,
            PostureFailureReason::PermissionDenied,
            posture_str,
            now,
        )
        .await?;
        return Err(AppServiceError::Posture(
            PostureFailureReason::PermissionDenied,
        ));
    }

    let posture = match Posture::parse(posture_str) {
        Ok(p) => p,
        Err(_) => {
            emit_rejected(
                store,
                actor.account_id,
                PostureFailureReason::UnsupportedPosture,
                posture_str,
                now,
            )
            .await?;
            return Err(AppServiceError::Posture(
                PostureFailureReason::UnsupportedPosture,
            ));
        }
    };

    let current = store.current_posture().await?;
    let expected_prev = current.as_ref().map(|r| r.posture);
    let from = expected_prev;

    let _record = store
        .set_posture(actor.account_id, posture, expected_prev)
        .await?;

    store
        .append_event(
            posture_envelope(
                PostureEventKind::PostureSet,
                &PostureSet {
                    actor: actor.account_id,
                    from,
                    to: posture,
                    at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(SetPostureResponse {
        current: posture_view(posture),
        change: PostureChangeView {
            actor: actor.account_id,
            at: now,
            from: from.unwrap_or(posture),
            to: posture,
        },
    })
}

async fn emit_rejected<S>(
    store: &S,
    actor: AccountId,
    reason: PostureFailureReason,
    requested_posture: &str,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            posture_envelope(
                PostureEventKind::PostureSetRejected,
                &PostureSetRejected {
                    actor,
                    reason,
                    requested_posture: requested_posture.to_owned(),
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}
