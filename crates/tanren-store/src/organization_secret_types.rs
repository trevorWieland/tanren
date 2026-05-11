//! Input/output types and error taxonomies for organization-secret store
//! operations. Split from `traits.rs` to keep each file under the
//! workspace line budget.

use chrono::{DateTime, Utc};
use secrecy::SecretString;
use tanren_configuration_secrets::{
    BaselineUsePolicy, OrganizationSecretId, OrganizationSecretName, SecretLifecycleStatus,
    SecretOwnerScope,
};
use tanren_identity_policy::{AccountId, OrgId};

use crate::{OrganizationSecretRecord, StoreError};

/// Input for [`AccountStore::create_organization_secret`](crate::AccountStore::create_organization_secret).
#[derive(Debug, Clone)]
pub struct CreateOrganizationSecretInput {
    /// Owning organization.
    pub org_id: OrgId,
    /// Validated secret name.
    pub name: OrganizationSecretName,
    /// Secret value to encrypt and store.
    pub secret_value: SecretString,
    /// Owner scope discriminator.
    pub owner_scope: SecretOwnerScope,
    /// Baseline use policy.
    pub use_policy: BaselineUsePolicy,
    /// Optional description.
    pub description: Option<String>,
    /// Optional provider.
    pub provider: Option<String>,
    /// Account creating the secret.
    pub creator_account_id: AccountId,
    /// Wall-clock time.
    pub now: DateTime<Utc>,
}

/// Output from [`AccountStore::create_organization_secret`](crate::AccountStore::create_organization_secret).
#[derive(Debug, Clone)]
pub struct CreateOrganizationSecretOutput {
    /// Persisted metadata record (no value).
    pub record: OrganizationSecretRecord,
}

/// Failure taxonomy for
/// [`AccountStore::create_organization_secret`](crate::AccountStore::create_organization_secret).
#[derive(Debug, thiserror::Error)]
pub enum CreateOrganizationSecretError {
    /// A secret with this name already exists in the organization.
    #[error("duplicate secret name")]
    DuplicateName,
    /// Unexpected database or encryption failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Input for [`AccountStore::update_organization_secret`](crate::AccountStore::update_organization_secret).
#[derive(Debug, Clone)]
pub struct UpdateOrganizationSecretInput {
    /// Owning organization.
    pub org_id: OrgId,
    /// Secret to update.
    pub secret_id: OrganizationSecretId,
    /// New secret value to encrypt and store.
    pub secret_value: SecretString,
    /// Optional updated description.
    pub description: Option<Option<String>>,
    /// Optional updated status.
    pub status: Option<SecretLifecycleStatus>,
    /// Optional updated use policy.
    pub use_policy: Option<BaselineUsePolicy>,
    /// Account performing the update.
    pub updater_account_id: AccountId,
    /// Wall-clock time.
    pub now: DateTime<Utc>,
}

/// Output from [`AccountStore::update_organization_secret`](crate::AccountStore::update_organization_secret).
#[derive(Debug, Clone)]
pub struct UpdateOrganizationSecretOutput {
    /// Updated metadata record (no value).
    pub record: OrganizationSecretRecord,
}

/// Failure taxonomy for
/// [`AccountStore::update_organization_secret`](crate::AccountStore::update_organization_secret).
#[derive(Debug, thiserror::Error)]
pub enum UpdateOrganizationSecretError {
    /// The referenced secret does not exist.
    #[error("secret not found")]
    NotFound,
    /// Unexpected database or encryption failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Input for [`AccountStore::remove_organization_secret`](crate::AccountStore::remove_organization_secret).
#[derive(Debug, Clone)]
pub struct RemoveOrganizationSecretInput {
    /// Owning organization.
    pub org_id: OrgId,
    /// Secret to remove.
    pub secret_id: OrganizationSecretId,
    /// Account performing the removal.
    pub remover_account_id: AccountId,
    /// Wall-clock time.
    pub now: DateTime<Utc>,
}

/// Output from [`AccountStore::remove_organization_secret`](crate::AccountStore::remove_organization_secret).
#[derive(Debug, Clone)]
pub struct RemoveOrganizationSecretOutput {
    /// Soft-deleted metadata record (no value).
    pub record: OrganizationSecretRecord,
}

/// Failure taxonomy for
/// [`AccountStore::remove_organization_secret`](crate::AccountStore::remove_organization_secret).
#[derive(Debug, thiserror::Error)]
pub enum RemoveOrganizationSecretError {
    /// The referenced secret does not exist.
    #[error("secret not found")]
    NotFound,
    /// Unexpected database failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Bounded page of organization-secret metadata records.
#[derive(Debug, Clone)]
pub struct ListOrganizationSecretsPage {
    /// Metadata records in this page.
    pub records: Vec<OrganizationSecretRecord>,
    /// Cursor for the next page, `None` if exhausted.
    pub next_cursor: Option<OrganizationSecretId>,
}

/// Request for [`AccountStore::list_organization_secrets`](crate::AccountStore::list_organization_secrets).
#[derive(Debug, Clone)]
pub struct ListOrganizationSecretsRequest {
    /// Organization to list secrets for.
    pub org_id: OrgId,
    /// Maximum records to return.
    pub limit: u64,
    /// Cursor from a previous page's `next_cursor`.
    pub cursor: Option<OrganizationSecretId>,
}

/// Output from [`AccountStore::resolve_organization_secret`](crate::AccountStore::resolve_organization_secret).
///
/// The resolved value is [`SecretString`] and must only be used at the
/// point of need — never logged, serialized, or projected.
#[derive(Debug, Clone)]
pub struct ResolveOrganizationSecretOutput {
    /// Metadata record for the resolved secret.
    pub record: OrganizationSecretRecord,
    /// The decrypted secret value.
    pub value: SecretString,
    /// Use policy governing this secret (for caller enforcement).
    pub use_policy: BaselineUsePolicy,
}

/// Failure taxonomy for
/// [`AccountStore::resolve_organization_secret`](crate::AccountStore::resolve_organization_secret).
#[derive(Debug, thiserror::Error)]
pub enum ResolveOrganizationSecretError {
    /// The referenced secret does not exist or has been deleted.
    #[error("secret not found")]
    NotFound,
    /// Use policy denies the caller from resolving this secret.
    #[error("use denied by policy")]
    UseDenied,
    /// Unexpected database or encryption failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Failure taxonomy for name/org-based secret lookups.
#[derive(Debug, thiserror::Error)]
pub enum OrganizationSecretLookupError {
    /// Unexpected database failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}
