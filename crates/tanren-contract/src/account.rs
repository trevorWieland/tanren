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
    AccountId, Email, Identifier, InvitationToken, OrgId, OrgPermissions, SessionToken,
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

/// Existing-account join request. An authenticated account accepts an
/// invitation to join an organization. Unlike [`AcceptInvitationRequest`]
/// (which creates a new account via R-0001), this flow operates on an
/// existing account identified by the caller's session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct JoinOrganizationRequest {
    /// Invitation token issued by the inviting organization.
    pub invitation_token: InvitationToken,
}

/// Successful existing-account join response. Carries the joined org,
/// the membership's organization-level permissions, the full set of
/// selectable organizations (so the caller can offer org switching via
/// R-0004), and an explicitly empty project-access grant list.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct JoinOrganizationResponse {
    /// Organization the account just joined.
    pub joined_org: OrgId,
    /// Organization-level permissions granted by the new membership.
    pub membership_permissions: OrgPermissions,
    /// All organization memberships selectable by this account after the
    /// join. The caller can use this list to offer org switching.
    pub selectable_organizations: Vec<OrgMembershipView>,
    /// Project-level access grants. Always empty on join — project access
    /// is governed by M-0031 and is not automatically granted by
    /// invitation acceptance.
    pub project_access_grants: Vec<ProjectAccessGrant>,
}

/// View of one organization membership for the selectable-org list
/// returned by [`JoinOrganizationResponse`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrgMembershipView {
    /// Organization the account is a member of.
    pub org_id: OrgId,
    /// Organization-level permissions for this membership.
    pub permissions: OrgPermissions,
}

/// Placeholder for a project-level access grant. Populated by the
/// project-access subsystem (M-0031); always empty during invitation
/// acceptance. The struct is intentionally a stub — fields will be added
/// by the project-access work that owns M-0031.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ProjectAccessGrant {}

/// Voluntary leave request. The authenticated member requests departure
/// from the specified organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct LeaveOrganizationRequest {
    /// Organization the caller wants to leave.
    pub org_id: OrgId,
}

/// Admin-initiated member removal request. The authenticated admin removes
/// another account from the specified organization.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct RemoveMemberRequest {
    /// Organization to remove the member from.
    pub org_id: OrgId,
    /// Account to remove from the organization.
    pub member_account_id: AccountId,
}

/// Placeholder for an in-flight work item surfaced before departure
/// completes. Detailed fields will be added by M-0042 (change history)
/// and M-0004 (assignment reassignment). The struct is intentionally a
/// stub so the departure response shape can represent preview-before-
/// completion without depending on project-access internals.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct InFlightWorkItem {}

/// Response for both voluntary leave and admin-initiated removal. Supports
/// a two-phase flow: preview-before-completion (surfaces in-flight work
/// with `completed = false`) and final departure (`completed = true`).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct MembershipDepartureResponse {
    /// `false` indicates a preview — in-flight work is surfaced but the
    /// departure has not been committed. `true` indicates the departure
    /// is final.
    pub completed: bool,
    /// In-flight work items that would be orphaned (preview) or are being
    /// handed off (completion).
    pub in_flight_work: Vec<InFlightWorkItem>,
    /// The organization the member departed. `None` during preview,
    /// `Some` after successful departure.
    pub departed_org: Option<OrgId>,
    /// All organization memberships selectable by the departing account
    /// after the departure completes. Used by callers to offer org
    /// switching.
    pub selectable_organizations: Vec<OrgMembershipView>,
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
    /// The invitation is addressed to a different account. The
    /// authenticated account does not match the invitation's target
    /// identifier.
    WrongAccount,
    /// The caller attempted a join operation without presenting a valid
    /// session. Interface layers map missing or expired sessions to this
    /// code before calling the handler.
    Unauthenticated,
    /// The account is not a member of the target organization.
    NotOrgMember,
    /// The authenticated account lacks the permission required for the
    /// requested operation (e.g. non-admin attempting member removal).
    PermissionDenied,
    /// The departure cannot proceed because the account is the last
    /// holder of administrative permissions in the organization. An org
    /// must always have at least one admin.
    LastAdminPermissionHolder,
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
            Self::WrongAccount => "wrong_account",
            Self::Unauthenticated => "unauthenticated",
            Self::NotOrgMember => "not_org_member",
            Self::PermissionDenied => "permission_denied",
            Self::LastAdminPermissionHolder => "last_admin_permission_holder",
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
            Self::WrongAccount => "The invitation is addressed to a different account.",
            Self::Unauthenticated => "Authentication is required to join an organization.",
            Self::NotOrgMember => "The account is not a member of the target organization.",
            Self::PermissionDenied => {
                "The authenticated account lacks permission for the requested operation."
            }
            Self::LastAdminPermissionHolder => {
                "The account is the last administrative-permission holder in the organization."
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
            Self::InvalidCredential | Self::Unauthenticated => 401,
            Self::ValidationFailed => 400,
            Self::InvitationNotFound | Self::NotOrgMember => 404,
            Self::InvitationExpired | Self::InvitationAlreadyConsumed => 410,
            Self::WrongAccount | Self::PermissionDenied | Self::LastAdminPermissionHolder => 403,
        }
    }
}
