//! Organization-secret command/response wire shapes.
//!
//! Secret values are accepted as [`SecretString`] on write and never
//! appear in response types. Write-side request types derive only
//! `Deserialize` — never `Serialize` — so secret values cannot leave
//! the process through Serde.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};
use tanren_configuration_secrets::{
    BaselineUsePolicy, OrganizationSecretId, OrganizationSecretName, SecretLifecycleStatus,
    SecretOwnerScope, SecretVersion,
};
use tanren_identity_policy::{AccountId, IdempotencyKey, OrgId, SessionToken, secret_serde};
use tanren_observation::{ClaimValueKind, CompletenessState, FreshnessState, VisibilityState};
use utoipa::{IntoParams, ToSchema};

pub const ORG_SECRET_EVENT_FAMILY: &str = "organization_secret";
pub const ORG_SECRET_CREATED_EVENT_KIND: &str = "organization_secret_created";
pub const ORG_SECRET_UPDATED_EVENT_KIND: &str = "organization_secret_updated";
pub const ORG_SECRET_DELETED_EVENT_KIND: &str = "organization_secret_deleted";
pub const ORG_SECRET_USED_EVENT_KIND: &str = "organization_secret_used";
pub const LIST_ORG_SECRETS_DEFAULT_LIMIT: u64 = 50;
pub const LIST_ORG_SECRETS_MAX_LIMIT: u64 = 100;

/// Sentinel: secret values are always redacted in interface responses.
///
/// Always serializes as `true`; deserialization rejects `false`.
/// A response with an unredacted secret value cannot be expressed
/// through this type (core invariant 3: secret values are never
/// projected).
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema, ToSchema)]
pub struct ValueRedacted;

impl Serialize for ValueRedacted {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(true)
    }
}

impl<'de> Deserialize<'de> for ValueRedacted {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = bool::deserialize(deserializer)?;
        if value {
            Ok(Self)
        } else {
            Err(serde::de::Error::custom(
                "value_redacted=false is not representable: secret values are never projected",
            ))
        }
    }
}

/// API body for creating an organization secret. Deserialize-only;
/// `secret_value` enters as [`SecretString`] and cannot be re-emitted.
#[derive(Debug, Clone, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationSecretApiRequest {
    /// Candidate secret name, validated by `OrganizationSecretName`.
    pub name: OrganizationSecretName,
    /// Plaintext secret value. This type does not implement `Serialize`,
    /// so the value can never leave the process through Serde.
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub secret_value: SecretString,
    /// Baseline use policy for the new secret.
    pub use_policy: BaselineUsePolicy,
    /// Optional description of the secret's purpose.
    pub description: Option<String>,
    /// Optional provider or service this secret is intended for.
    pub provider: Option<String>,
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Create-organization-secret request (authenticated). Deserialize-only
/// so the inner secret value remains receive-only.
#[derive(Debug, Clone, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationSecretRequest {
    pub session_token: SessionToken,
    pub account_id: AccountId,
    pub org_id: OrgId,
    pub body: CreateOrganizationSecretApiRequest,
}

/// Create-organization-secret response. Metadata only — no value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct CreateOrganizationSecretResponse {
    pub secret: OrganizationSecretView,
    pub proof_link: OrganizationSecretProofLink,
    pub source_link: OrganizationSecretSourceLink,
    pub source_event: Option<OrganizationSecretEventReference>,
}

/// API body for updating an organization secret. Deserialize-only;
/// `secret_value` enters as [`SecretString`] and cannot be re-emitted.
#[derive(Debug, Clone, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateOrganizationSecretApiRequest {
    /// Plaintext secret value. This type does not implement `Serialize`,
    /// so the value can never leave the process through Serde.
    #[serde(deserialize_with = "secret_serde::deserialize_password")]
    #[schemars(with = "String")]
    #[schema(value_type = String, format = Password)]
    pub secret_value: SecretString,
    /// Updated description, if changing.
    pub description: Option<String>,
    /// Updated use policy, if changing.
    pub use_policy: Option<BaselineUsePolicy>,
    /// Stable client idempotency key.
    pub idempotency_key: Option<IdempotencyKey>,
}

/// Update-organization-secret request (authenticated). Deserialize-only
/// so the inner secret value remains receive-only.
#[derive(Debug, Clone, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateOrganizationSecretRequest {
    pub session_token: SessionToken,
    pub account_id: AccountId,
    pub org_id: OrgId,
    pub secret_id: OrganizationSecretId,
    pub body: UpdateOrganizationSecretApiRequest,
}

/// Update-organization-secret response. Metadata only — no value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UpdateOrganizationSecretResponse {
    pub secret: OrganizationSecretView,
    pub proof_link: OrganizationSecretProofLink,
    pub source_link: OrganizationSecretSourceLink,
    pub source_event: Option<OrganizationSecretEventReference>,
}

/// Delete-organization-secret request (authenticated).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeleteOrganizationSecretRequest {
    pub session_token: SessionToken,
    pub account_id: AccountId,
    pub org_id: OrgId,
    pub secret_id: OrganizationSecretId,
}

/// Delete-organization-secret response. Metadata only — no value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct DeleteOrganizationSecretResponse {
    pub secret: OrganizationSecretView,
    pub proof_link: OrganizationSecretProofLink,
    pub source_link: OrganizationSecretSourceLink,
    pub source_event: Option<OrganizationSecretEventReference>,
}

/// Read-organization-secret request (authenticated).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ReadOrganizationSecretRequest {
    pub session_token: SessionToken,
    pub account_id: AccountId,
    pub org_id: OrgId,
    pub secret_id: OrganizationSecretId,
}

/// Read-organization-secret response. Metadata only — no value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ReadOrganizationSecretResponse {
    /// Secret metadata projection (no value).
    pub secret: OrganizationSecretView,
    /// Explicit redaction marker. Always true.
    pub value_redacted: ValueRedacted,
    /// Proof link for the read event.
    pub proof_link: OrganizationSecretProofLink,
    /// Source link for the read event.
    pub source_link: OrganizationSecretSourceLink,
    /// Read-model freshness metadata.
    pub freshness: OrganizationSecretReadModelFreshness,
}

/// Query parameters for `GET /organizations/{org_id}/secrets`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema, IntoParams)]
pub struct ListOrganizationSecretsApiQuery {
    pub limit: Option<u64>,
    pub cursor: Option<String>,
}

/// List-organization-secrets request (authenticated).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationSecretsRequest {
    pub session_token: SessionToken,
    pub account_id: AccountId,
    pub org_id: OrgId,
    pub limit: Option<u64>,
    pub cursor: Option<String>,
}

impl ListOrganizationSecretsRequest {
    /// Build from authenticated transport context and query parameters.
    #[must_use]
    pub fn from_api_query(
        session_token: SessionToken,
        account_id: AccountId,
        org_id: OrgId,
        query: &ListOrganizationSecretsApiQuery,
    ) -> Self {
        Self {
            session_token,
            account_id,
            org_id,
            limit: query.limit,
            cursor: query.cursor.clone(),
        }
    }
}

/// List-organization-secrets response. Metadata only — no values.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct ListOrganizationSecretsResponse {
    pub secrets: Vec<OrganizationSecretSummaryView>,
    pub next_cursor: Option<String>,
    pub source_link: OrganizationSecretSourceLink,
    pub freshness: OrganizationSecretReadModelFreshness,
}

/// Full metadata view for a single organization secret. No value field.
///
/// The `value_redacted` marker is always [`ValueRedacted`] — serializes
/// as `true`, deserialization rejects `false` — encoding the invariant
/// that secret values are never projected through response types.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretView {
    pub id: OrganizationSecretId,
    pub org_id: OrgId,
    pub name: OrganizationSecretName,
    pub owner_scope: SecretOwnerScope,
    pub status: SecretLifecycleStatus,
    pub use_policy: BaselineUsePolicy,
    pub version: SecretVersion,
    pub description: Option<String>,
    pub provider: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Secret value is always redacted in interface responses.
    pub value_redacted: ValueRedacted,
}

/// Summary view for list responses — lighter than [`OrganizationSecretView`].
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretSummaryView {
    pub id: OrganizationSecretId,
    pub name: OrganizationSecretName,
    pub status: SecretLifecycleStatus,
    pub use_policy: BaselineUsePolicy,
    pub version: SecretVersion,
    pub provider: Option<String>,
    /// Secret value is always redacted in interface responses.
    pub value_redacted: ValueRedacted,
}

/// Stable proof reference clients can render without event-log probing.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretProofLink {
    /// Event family for proof linkage.
    pub event_family: String,
    /// Event kind for proof linkage.
    pub event_kind: String,
}

/// Stable source reference for secret lifecycle events.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretSourceLink {
    pub event_family: String,
    pub event_kind: String,
}

/// Concrete event reference from the canonical event log.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretEventReference {
    pub event_family: String,
    pub event_kind: String,
    pub event_id: String,
    pub cursor: String,
    pub occurred_at: DateTime<Utc>,
}

/// Freshness and provenance metadata for organization-secret read models.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretReadModelFreshness {
    pub visibility: VisibilityState,
    pub completeness: CompletenessState,
    pub freshness: FreshnessState,
    pub claim_value_kind: ClaimValueKind,
}

/// Cross-interface failure taxonomy for organization-secret operations.
/// Every interface maps into this taxonomy so callers match on `code`
/// regardless of transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OrganizationSecretFailureReason {
    /// Request requires authentication; no valid session supplied.
    AuthRequired,
    /// Actor is authenticated but lacks permission.
    PermissionDenied,
    /// User-supplied input failed validation.
    ValidationFailed,
    /// Referenced secret does not exist.
    NotFound,
    /// Operation conflicts with current state (e.g., duplicate name).
    Conflict,
    /// Internal error.
    InternalError,
}

impl OrganizationSecretFailureReason {
    /// Stable wire `code` for this failure.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::AuthRequired => "auth_required",
            Self::PermissionDenied => "permission_denied",
            Self::ValidationFailed => "validation_failed",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::InternalError => "internal_error",
        }
    }

    /// Human-readable wire `summary` for this failure.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::AuthRequired => {
                "The request requires authentication and the supplied session is missing or expired."
            }
            Self::PermissionDenied => {
                "The authenticated actor lacks permission to perform this action."
            }
            Self::ValidationFailed => {
                "The submitted input did not satisfy contract-level validation."
            }
            Self::NotFound => "The referenced organization secret does not exist.",
            Self::Conflict => {
                "The operation conflicts with the current state (e.g., duplicate secret name)."
            }
            Self::InternalError => "An internal error prevented the operation from completing.",
        }
    }

    /// Recommended HTTP status for api/mcp surfaces.
    #[must_use]
    pub const fn http_status(self) -> u16 {
        match self {
            Self::AuthRequired => 401,
            Self::PermissionDenied => 403,
            Self::ValidationFailed => 400,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::InternalError => 500,
        }
    }
}

/// Shared failure body for organization-secret error responses.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretFailureBody {
    pub code: String,
    pub summary: String,
    pub reason: OrganizationSecretFailureReason,
}

impl OrganizationSecretFailureBody {
    /// Construct a failure body from a failure reason.
    #[must_use]
    pub fn from_reason(reason: OrganizationSecretFailureReason) -> Self {
        Self {
            code: reason.code().to_owned(),
            summary: reason.summary().to_owned(),
            reason,
        }
    }
}

/// Event payload for `organization_secret_created`. Metadata only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretCreatedEvent {
    pub secret_id: OrganizationSecretId,
    pub org_id: OrgId,
    /// Secret name (metadata, not value).
    pub name: OrganizationSecretName,
    pub owner_scope: SecretOwnerScope,
    pub use_policy: BaselineUsePolicy,
    pub version: SecretVersion,
    pub creator_account_id: AccountId,
    pub created_at: DateTime<Utc>,
}

/// Event payload for `organization_secret_updated`. Metadata only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretUpdatedEvent {
    pub secret_id: OrganizationSecretId,
    pub org_id: OrgId,
    pub name: OrganizationSecretName,
    pub version: SecretVersion,
    pub updater_account_id: AccountId,
    pub updated_at: DateTime<Utc>,
}

/// Event payload for `organization_secret_deleted`. Metadata only.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretDeletedEvent {
    pub secret_id: OrganizationSecretId,
    pub org_id: OrgId,
    pub name: OrganizationSecretName,
    pub deleter_account_id: AccountId,
    pub deleted_at: DateTime<Utc>,
}

/// Request to use (resolve) an organization secret's value. The resolved
/// value is consumed by the service layer and never returned to the caller.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UseOrganizationSecretRequest {
    /// Session proving the caller is authenticated.
    pub session_token: SessionToken,
    /// Account requesting the use.
    pub account_id: AccountId,
    /// Organization that owns the secret.
    pub org_id: OrgId,
    /// Secret to resolve and use.
    pub secret_id: OrganizationSecretId,
}

/// Response from a use operation. Metadata only — the resolved value was
/// consumed by the service layer. `value_redacted` is always `true`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct UseOrganizationSecretResponse {
    /// Secret metadata after the use.
    pub secret: OrganizationSecretView,
    /// The value was consumed in-memory and is permanently redacted.
    pub value_redacted: ValueRedacted,
    /// Proof link for the use event.
    pub proof_link: OrganizationSecretProofLink,
    /// Source link for the use event.
    pub source_link: OrganizationSecretSourceLink,
    /// Concrete source event reference if the use event was appended.
    pub source_event: Option<OrganizationSecretEventReference>,
}

/// Event payload for `organization_secret_used`. Metadata only — no value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, ToSchema)]
pub struct OrganizationSecretUsedEvent {
    /// Secret identifier.
    pub secret_id: OrganizationSecretId,
    /// Owning organization.
    pub org_id: OrgId,
    /// Secret name (metadata, not value).
    pub name: OrganizationSecretName,
    /// Version that was used.
    pub version: SecretVersion,
    /// Account that triggered the use.
    pub actor_account_id: AccountId,
    /// When the use occurred.
    pub used_at: DateTime<Utc>,
}
