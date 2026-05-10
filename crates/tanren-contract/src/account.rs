//! Account command/response wire shapes.
//!
//! These types are the request/response surface used by the api, mcp,
//! cli, tui, and web client when callers create or sign in to a Tanren
//! account. They live in `tanren-contract` because every interface
//! binary serialises the same shapes — keeping them here is the
//! architectural guarantee that the surfaces stay equivalent.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_identity_policy::secret_serde;
use tanren_identity_policy::{AccountId, Email, Identifier, InvitationToken, OrgId, SessionToken};
use utoipa::ToSchema;

/// Self-signup request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SignUpRequest {
    /// Email address that will own the new account.
    pub email: Email,
    /// Account password.
    // Hashed by the handler before persistence. Wrapped in SecretString
    // so accidental Debug / Serialize calls do not leak the credential.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_redacted"
    )]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub password: SecretString,
    /// Human-readable display name for the new account.
    pub display_name: String,
}

/// Successful sign-up response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SignUpResponse {
    /// View of the freshly created account.
    pub account: AccountView,
    /// Session minted for the new account.
    pub session: SessionView,
}

/// Sign-in request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SignInRequest {
    /// Email of the account being signed in to.
    pub email: Email,
    /// Account password.
    // Verified against the stored hash.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_redacted"
    )]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub password: SecretString,
}

/// Successful sign-in response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SignInResponse {
    /// View of the signed-in account.
    pub account: AccountView,
    /// Newly minted session.
    pub session: SessionView,
}

/// Invitation-acceptance request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct AcceptInvitationRequest {
    /// Invitation token issued by the inviting organization.
    pub invitation_token: InvitationToken,
    /// Email the invitee chooses for the new account.
    pub email: Email,
    /// Plaintext password for the new account.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_redacted"
    )]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub password: SecretString,
    /// Display name for the new account.
    pub display_name: String,
}

/// Successful invitation-acceptance response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct AcceptInvitationResponse {
    /// View of the newly created account.
    pub account: AccountView,
    /// Newly minted session.
    pub session: SessionView,
    /// Organization the new account joined as a result of this acceptance.
    pub joined_org: OrgId,
}

/// External-facing view of a Tanren account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct AccountView {
    /// Stable account id.
    pub id: AccountId,
    /// User-facing identifier (email).
    pub identifier: Identifier,
    /// Display name.
    pub display_name: String,
    /// Owning organization id — `None` for personal (self-signup) accounts.
    pub org: Option<OrgId>,
}

/// External-facing view of a session token.
// The token is opaque to all callers; only the issuer understands
// its internal shape.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SessionView {
    /// Account this session is bound to.
    pub account_id: AccountId,
    #[serde(
        serialize_with = "secret_serde::serialize_session_token_redacted",
        deserialize_with = "secret_serde::deserialize_session_token"
    )]
    /// Opaque session token.
    pub token: SessionToken,
    /// Wall-clock time at which the session expires.
    pub expires_at: DateTime<Utc>,
}

/// Cookie-transport session envelope for API/web account responses.
///
/// Session tokens are written to an `HttpOnly` cookie and are never
/// returned in these response bodies.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum CookieSessionEnvelope {
    /// Cookie-bound session metadata.
    Cookie {
        /// Account this session is bound to.
        account_id: AccountId,
        /// Wall-clock time at which the session expires.
        expires_at: DateTime<Utc>,
    },
}

impl CookieSessionEnvelope {
    /// Project a [`SessionView`] into the cookie envelope.
    #[must_use]
    pub fn from_session_view(view: &SessionView) -> Self {
        Self::Cookie {
            account_id: view.account_id,
            expires_at: view.expires_at,
        }
    }
}

/// Transport-aware projection of a freshly minted session.
///
/// The `@web` and `@api` surfaces deliver session tokens via an
/// `HttpOnly + Secure + SameSite=Strict` cookie set by the API; the body
/// only exposes `account_id` + `expires_at` (`Cookie` variant). The
/// `@cli`, `@mcp`, and `@tui` surfaces have no cookie jar — they receive
/// the token in the response body (`Bearer` variant). The discriminator
/// is transport, not identity.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum SessionEnvelope {
    /// Cookie-bound session for `@web` + `@api`. The token is set by the
    /// server via `Set-Cookie` and never appears in the response body.
    Cookie {
        /// Account this session is bound to.
        account_id: AccountId,
        /// Wall-clock time at which the session expires.
        expires_at: DateTime<Utc>,
    },
    /// Bearer-token session for `@cli` + `@mcp` + `@tui`. The opaque
    /// token is returned in the body; clients keep it in their session
    /// store / keyring.
    Bearer {
        /// Account this session is bound to.
        account_id: AccountId,
        /// Wall-clock time at which the session expires.
        expires_at: DateTime<Utc>,
        #[serde(
            serialize_with = "secret_serde::serialize_session_token_redacted",
            deserialize_with = "secret_serde::deserialize_session_token"
        )]
        /// Opaque session token.
        token: SessionToken,
    },
}

impl SessionEnvelope {
    /// Project a [`SessionView`] into the cookie-transport envelope (no
    /// token in body — it ships in the `Set-Cookie` header).
    #[must_use]
    pub fn cookie(view: &SessionView) -> Self {
        Self::Cookie {
            account_id: view.account_id,
            expires_at: view.expires_at,
        }
    }

    /// Project a [`SessionView`] into the bearer-transport envelope (token
    /// in body — for clients without a cookie jar).
    #[must_use]
    pub fn bearer(view: &SessionView) -> Self {
        Self::Bearer {
            account_id: view.account_id,
            expires_at: view.expires_at,
            token: view.token.clone(),
        }
    }
}

/// Bearer-transport session envelope for non-cookie account responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct BearerSessionEnvelope {
    /// Account this session is bound to.
    pub account_id: AccountId,
    /// Wall-clock time at which the session expires.
    pub expires_at: DateTime<Utc>,
    #[serde(
        serialize_with = "secret_serde::serialize_session_token_expose",
        deserialize_with = "secret_serde::deserialize_session_token"
    )]
    /// Opaque session token.
    pub token: SessionToken,
}

impl BearerSessionEnvelope {
    /// Project a [`SessionView`] into the bearer envelope.
    #[must_use]
    pub fn from_session_view(view: &SessionView) -> Self {
        Self {
            account_id: view.account_id,
            expires_at: view.expires_at,
            token: view.token.clone(),
        }
    }
}

/// Bearer-transport sign-up response used by `@cli`, `@mcp`, and `@tui`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SignUpResponseBearer {
    /// View of the freshly created account.
    pub account: AccountView,
    /// Explicit bearer envelope (token emitted here only).
    pub session: BearerSessionEnvelope,
}

impl SignUpResponseBearer {
    /// Convert app-service output into an explicit bearer transport payload.
    #[must_use]
    pub fn from_sign_up_response(response: &SignUpResponse) -> Self {
        Self {
            account: response.account.clone(),
            session: BearerSessionEnvelope::from_session_view(&response.session),
        }
    }
}

/// Bearer-transport sign-in response used by `@cli`, `@mcp`, and `@tui`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SignInResponseBearer {
    /// View of the signed-in account.
    pub account: AccountView,
    /// Explicit bearer envelope (token emitted here only).
    pub session: BearerSessionEnvelope,
}

impl SignInResponseBearer {
    /// Convert app-service output into an explicit bearer transport payload.
    #[must_use]
    pub fn from_sign_in_response(response: &SignInResponse) -> Self {
        Self {
            account: response.account.clone(),
            session: BearerSessionEnvelope::from_session_view(&response.session),
        }
    }
}

/// Bearer-transport invitation-acceptance response for non-cookie clients.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct AcceptInvitationResponseBearer {
    /// View of the newly created account.
    pub account: AccountView,
    /// Explicit bearer envelope (token emitted here only).
    pub session: BearerSessionEnvelope,
    /// Organization the new account joined.
    pub joined_org: OrgId,
}

impl AcceptInvitationResponseBearer {
    /// Convert app-service output into an explicit bearer transport payload.
    #[must_use]
    pub fn from_accept_invitation_response(response: &AcceptInvitationResponse) -> Self {
        Self {
            account: response.account.clone(),
            session: BearerSessionEnvelope::from_session_view(&response.session),
            joined_org: response.joined_org,
        }
    }
}

/// Closed taxonomy of account-flow failures.
///
/// Maps onto the shared `{code, summary}` error body documented in
/// `docs/architecture/subsystems/interfaces.md` "Error Taxonomy". Every
/// interface (api/mcp/cli/tui/web) projects an `AccountFailureReason`
/// into the same wire shape so callers can match on `code` regardless of
/// transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum AccountFailureReason {
    /// The submitted identifier is already in use by another account.
    DuplicateIdentifier,
    /// The submitted credentials did not match a stored credential.
    InvalidCredential,
    /// User-supplied input failed validation before any verification
    /// could run (empty password, malformed email, ...). Distinct from
    /// `InvalidCredential` so callers can tell "your inputs are
    /// malformed" apart from "your credentials don't match".
    ValidationFailed,
    /// Invitation token does not correspond to any known invitation.
    InvitationNotFound,
    /// Invitation token has expired.
    InvitationExpired,
    /// Invitation token has already been accepted or revoked.
    InvitationAlreadyConsumed,
}

/// Wire-visible account failure codes for `{code, summary}` error bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountFailureCode {
    DuplicateIdentifier,
    InvalidCredential,
    ValidationFailed,
    InvitationNotFound,
    InvitationExpired,
    InvitationAlreadyConsumed,
    AuthRequired,
    InternalError,
}

impl From<AccountFailureReason> for AccountFailureCode {
    fn from(reason: AccountFailureReason) -> Self {
        match reason {
            AccountFailureReason::DuplicateIdentifier => Self::DuplicateIdentifier,
            AccountFailureReason::InvalidCredential => Self::InvalidCredential,
            AccountFailureReason::ValidationFailed => Self::ValidationFailed,
            AccountFailureReason::InvitationNotFound => Self::InvitationNotFound,
            AccountFailureReason::InvitationExpired => Self::InvitationExpired,
            AccountFailureReason::InvitationAlreadyConsumed => Self::InvitationAlreadyConsumed,
        }
    }
}

impl AccountFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::DuplicateIdentifier => "duplicate_identifier",
            Self::InvalidCredential => "invalid_credential",
            Self::ValidationFailed => "validation_failed",
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
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
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
            Self::ValidationFailed => 400,
            Self::InvitationNotFound => 404,
            Self::InvitationExpired | Self::InvitationAlreadyConsumed => 410,
        }
    }
}
