//! Posture command/response wire shapes.
//!
//! Wire shapes for the deployment-posture decision surface. These types are
//! the request/response surface used by the api, mcp, cli, tui, and web
//! client when callers inspect or change the installation's deployment
//! posture. They live in `tanren-contract` because every interface binary
//! serialises the same shapes — keeping them here is the architectural
//! guarantee that the surfaces stay equivalent.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tanren_domain::{CapabilitySummary, Posture};
use tanren_identity_policy::AccountId;
use utoipa::ToSchema;

/// Closed taxonomy of posture-flow failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects a `PostureFailureReason`
/// into the same wire shape so callers can match on `code` regardless of
/// transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PostureFailureReason {
    /// The requested posture is not supported by this build or configuration.
    UnsupportedPosture,
    /// The caller does not have permission to view or change the posture.
    PermissionDenied,
    /// The installation has not completed the posture-selection step of
    /// first-run.
    NotConfigured,
}

impl PostureFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedPosture => "unsupported_posture",
            Self::PermissionDenied => "permission_denied",
            Self::NotConfigured => "not_configured",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::UnsupportedPosture => "The requested deployment posture is not supported.",
            Self::PermissionDenied => {
                "You do not have permission to perform this posture operation."
            }
            Self::NotConfigured => {
                "No deployment posture has been configured for this installation."
            }
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces. Centralized so every transport reports the
    /// same status for the same failure code.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::UnsupportedPosture => 422,
            Self::PermissionDenied => 403,
            Self::NotConfigured => 424,
        }
    }
}

/// External-facing view of the installation's deployment posture and its
/// capability availability.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PostureView {
    /// The deployment posture.
    pub posture: Posture,
    /// Capability availability summary for this posture.
    pub capabilities: Vec<CapabilitySummary>,
}

/// Attribution record for a posture change.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct PostureChangeView {
    /// Account that initiated the change.
    pub actor: AccountId,
    /// Wall-clock time the change was applied.
    pub at: DateTime<Utc>,
    /// Previous posture.
    pub from: Posture,
    /// New posture.
    pub to: Posture,
}

/// Response listing all available deployment postures with their capability
/// summaries.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListPosturesResponse {
    /// Available posture options, each with capability details.
    pub postures: Vec<PostureView>,
}

/// Response returning the installation's current deployment posture.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct GetPostureResponse {
    /// Current posture and capability summary.
    pub current: PostureView,
}

/// Request to set the installation's deployment posture.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetPostureRequest {
    /// The desired deployment posture.
    pub posture: Posture,
}

/// Response confirming the posture was changed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SetPostureResponse {
    /// Updated posture and capability summary.
    pub current: PostureView,
    /// Attribution for the change that was applied.
    pub change: PostureChangeView,
}
