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
use tanren_identity_policy::{
    AccountId, Email, Identifier, InvitationToken, OrgId, ProjectId, SessionToken,
};
use utoipa::ToSchema;

/// Self-signup request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SignUpRequest {
    /// Email address that will own the new account. Lower-cased + trimmed
    /// during validation.
    pub email: Email,
    /// Plaintext password. Hashed by the handler before persistence.
    /// Wrapped in `SecretString` so accidental `Debug` / `Serialize`
    /// calls do not leak the credential.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_expose"
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
    /// Plaintext password — verified against the stored hash.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_expose"
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
    /// Email the invitee chooses for the new account. (Subsequent PRs
    /// finalize the email-from-invitation flow; for PR 3 the field is
    /// already first-class on the wire shape.)
    pub email: Email,
    /// Plaintext password for the new account.
    #[serde(
        deserialize_with = "secret_serde::deserialize_password",
        serialize_with = "secret_serde::serialize_password_expose"
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

/// External-facing view of a session token. The token is opaque to all
/// callers; only the issuer (the api/cli/mcp/tui binary that signed it)
/// understands its internal shape.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SessionView {
    /// Account this session is bound to.
    pub account_id: AccountId,
    /// Opaque session token.
    pub token: SessionToken,
    /// Wall-clock time at which the session expires.
    pub expires_at: DateTime<Utc>,
}

/// Transport-aware projection of a freshly minted session.
///
/// The `@web` and `@api` surfaces deliver session tokens via an
/// `HttpOnly + Secure + SameSite=Strict` cookie set by the API; the body
/// only exposes `account_id` + `expires_at` (`Cookie` variant). The
/// `@cli`, `@mcp`, and `@tui` surfaces have no cookie jar — they receive
/// the token in the response body (`Bearer` variant). Subsequent PRs map
/// `SessionView` → `SessionEnvelope` per surface inside each binary
/// (cookie session lands in PR 8). The discriminator is the transport,
/// not the user.
///
/// See `docs/architecture/subsystems/interfaces.md` § "Canonical session,
/// error, `OpenAPI`, and design-token decisions" and
/// `profiles/rust-cargo/architecture/cookie-session.md`.
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

/// View of an organization the caller belongs to, used by the
/// organization-switcher UI surface.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationMembershipView {
    /// Organization id.
    pub org_id: OrgId,
    /// Human-readable organization name.
    pub org_name: String,
}

/// Organization switcher state — lists organizations the caller belongs
/// to and identifies which one is currently active. Personal accounts
/// with zero org memberships receive an empty `memberships` vector and
/// `active_org: None`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSwitcher {
    /// Organizations the account is a member of.
    pub memberships: Vec<OrganizationMembershipView>,
    /// Currently active organization, if any.
    pub active_org: Option<OrgId>,
}

/// Request to switch the active organization for the caller's account.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SwitchActiveOrganizationRequest {
    /// Target organization to make active.
    pub org_id: OrgId,
}

/// Successful switch-active-organization response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct SwitchActiveOrganizationResponse {
    /// Updated account view reflecting the new active organization.
    pub account: AccountView,
}

/// Request to list projects scoped to a specific organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationProjectsRequest {
    /// Organization whose projects to list.
    pub org_id: OrgId,
}

/// External-facing view of a project within an organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectView {
    /// Stable project id.
    pub id: ProjectId,
    /// Human-readable project name.
    pub name: String,
    /// Owning organization.
    pub org: OrgId,
}

/// Successful organization-scoped project list response.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationProjectsResponse {
    /// Projects belonging to the requested organization.
    pub projects: Vec<ProjectView>,
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
    /// Caller is not a member of the target organization.
    OrganizationNotMember,
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
            Self::OrganizationNotMember => "organization_not_member",
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
            Self::OrganizationNotMember => {
                "The account is not a member of the target organization."
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
            Self::OrganizationNotMember => 403,
        }
    }
}
