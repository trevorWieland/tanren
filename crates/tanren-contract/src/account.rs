//! Account command/response wire shapes.
//!
//! These types are the request/response surface used by the api, mcp,
//! cli, tui, and web client when callers create or sign in to a Tanren
//! account. They live in `tanren-contract` because every interface
//! binary serialises the same shapes — keeping them here is the
//! architectural guarantee that the surfaces stay equivalent.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Self-signup request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SignUpRequest {
    /// Email address that will own the new account. Lower-cased + trimmed
    /// during validation.
    pub email: String,
    /// Plaintext password. Hashed by the handler before persistence.
    pub password: String,
    /// Human-readable display name for the new account.
    pub display_name: String,
}

/// Successful sign-up response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SignUpResponse {
    /// View of the freshly created account.
    pub account: AccountView,
    /// Session minted for the new account.
    pub session: SessionView,
}

/// Sign-in request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SignInRequest {
    /// Email of the account being signed in to.
    pub email: String,
    /// Plaintext password — verified against the stored hash.
    pub password: String,
}

/// Successful sign-in response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SignInResponse {
    /// View of the signed-in account.
    pub account: AccountView,
    /// Newly minted session.
    pub session: SessionView,
}

/// Invitation-acceptance request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AcceptInvitationRequest {
    /// Invitation token issued by the inviting organization.
    pub invitation_token: String,
    /// Plaintext password for the new account.
    pub password: String,
    /// Display name for the new account.
    pub display_name: String,
}

/// Successful invitation-acceptance response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AcceptInvitationResponse {
    /// View of the newly created account.
    pub account: AccountView,
    /// Newly minted session.
    pub session: SessionView,
    /// Organization the new account joined as a result of this acceptance.
    pub joined_org: Uuid,
}

/// External-facing view of a Tanren account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AccountView {
    /// Stable account id.
    pub id: Uuid,
    /// User-facing identifier (email).
    pub identifier: String,
    /// Display name.
    pub display_name: String,
    /// Owning organization id — `None` for personal (self-signup) accounts.
    pub org: Option<Uuid>,
}

/// External-facing view of a session token. The token is opaque to all
/// callers; only the issuer (the api/cli/mcp/tui binary that signed it)
/// understands its internal shape.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionView {
    /// Account this session is bound to.
    pub account_id: Uuid,
    /// Opaque session token.
    pub token: String,
}

/// Closed taxonomy of account-flow failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects an `AccountFailureReason`
/// into the same wire shape so callers can match on `code` regardless of
/// transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccountFailureReason {
    /// The submitted identifier is already in use by another account.
    DuplicateIdentifier,
    /// The submitted credentials did not match a stored credential.
    InvalidCredential,
    /// Invitation token does not correspond to any known invitation.
    InvitationNotFound,
    /// Invitation token has expired.
    InvitationExpired,
    /// Invitation token has already been accepted or revoked.
    InvitationAlreadyConsumed,
}

impl AccountFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DuplicateIdentifier => "duplicate_identifier",
            Self::InvalidCredential => "invalid_credential",
            Self::InvitationNotFound => "invitation_not_found",
            Self::InvitationExpired => "invitation_expired",
            Self::InvitationAlreadyConsumed => "invitation_already_consumed",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::DuplicateIdentifier => "An account already exists for the supplied identifier.",
            Self::InvalidCredential => {
                "The supplied credentials are invalid or did not match an account."
            }
            Self::InvitationNotFound => "The invitation token does not match any known invitation.",
            Self::InvitationExpired => "The invitation has expired and can no longer be accepted.",
            Self::InvitationAlreadyConsumed => {
                "The invitation has already been accepted or was revoked."
            }
        }
    }

    /// Recommended HTTP status for the failure when projected over the
    /// api / mcp surfaces. Centralized so every transport reports the
    /// same status for the same failure code.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::DuplicateIdentifier => 409,
            Self::InvalidCredential => 401,
            Self::InvitationNotFound => 404,
            Self::InvitationExpired | Self::InvitationAlreadyConsumed => 410,
        }
    }
}
