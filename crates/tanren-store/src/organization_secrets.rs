//! Organization-secret persistence: encryption adapter, record conversion,
//! and store method implementations. Keeps `lib.rs` below the file-size budget.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use secrecy::{ExposeSecret, SecretString};
use tanren_configuration_secrets::{
    BaselineUsePolicy, OrganizationSecretId, OrganizationSecretName, SecretLifecycleStatus,
    SecretOwnerScope, SecretVersion,
};
use tanren_identity_policy::OrgId;

use crate::entity;
use crate::organization_secret_types::{
    CreateOrganizationSecretError, CreateOrganizationSecretInput, CreateOrganizationSecretOutput,
    ListOrganizationSecretsPage, ListOrganizationSecretsRequest, OrganizationSecretLookupError,
    RemoveOrganizationSecretError, RemoveOrganizationSecretInput, RemoveOrganizationSecretOutput,
    ResolveOrganizationSecretError, ResolveOrganizationSecretOutput, UpdateOrganizationSecretError,
    UpdateOrganizationSecretInput, UpdateOrganizationSecretOutput,
};
use crate::{OrganizationSecretRecord, StoreError};
// Encryption adapter

/// Installation-managed 256-bit AES key for encrypting organization secret
/// values at rest. The key material lives outside the database; this struct
/// wraps the parsed `Aes256Gcm` cipher.
#[derive(Clone)]
pub struct SecretEncryptionKey {
    cipher: Aes256Gcm,
    key_id: String,
}

impl std::fmt::Debug for SecretEncryptionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretEncryptionKey")
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

impl SecretEncryptionKey {
    /// Construct from raw 32-byte key material and a human-readable key id.
    ///
    /// Returns [`StoreError`] if the key material length is not exactly 32 bytes.
    pub fn from_bytes(key_id: String, bytes: &[u8]) -> Result<Self, StoreError> {
        if bytes.len() != 32 {
            return Err(StoreError::Database(DbErr::Custom(
                "AES-256-GCM key must be exactly 32 bytes".into(),
            )));
        }
        let key = Key::<Aes256Gcm>::from_slice(bytes);
        Ok(Self {
            cipher: Aes256Gcm::new(key),
            key_id,
        })
    }

    /// Encrypt a plaintext secret value. Returns ciphertext and nonce.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), StoreError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| StoreError::SecretStoreUnavailable)?;
        Ok((ciphertext, nonce.to_vec()))
    }

    /// Decrypt a ciphertext with the given nonce.
    pub fn decrypt(&self, nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, StoreError> {
        let nonce = Nonce::from_slice(nonce);
        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| StoreError::SecretStoreUnavailable)
    }

    /// The key id stored alongside each encrypted value for key rotation support.
    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
}

// Record conversion helpers

/// Internal helper: parse a DB-stored secret name string into a validated newtype.
pub(crate) fn parse_db_secret_name(raw: &str) -> Result<OrganizationSecretName, StoreError> {
    OrganizationSecretName::parse(raw).map_err(|err| {
        StoreError::Database(DbErr::Custom(format!(
            "organization_secret_name invariant: {err}"
        )))
    })
}

pub(crate) fn parse_db_owner_scope(raw: &str) -> Result<SecretOwnerScope, StoreError> {
    raw.parse::<SecretOwnerScope>()
        .map_err(|()| StoreError::Database(DbErr::Custom(format!("unknown owner_scope: {raw}"))))
}

pub(crate) fn parse_db_lifecycle_status(raw: &str) -> Result<SecretLifecycleStatus, StoreError> {
    raw.parse::<SecretLifecycleStatus>().map_err(|()| {
        StoreError::Database(DbErr::Custom(format!("unknown lifecycle status: {raw}")))
    })
}

pub(crate) fn parse_db_use_policy(raw: &str) -> Result<BaselineUsePolicy, StoreError> {
    raw.parse::<BaselineUsePolicy>()
        .map_err(|()| StoreError::Database(DbErr::Custom(format!("unknown use_policy: {raw}"))))
}

pub(crate) fn parse_db_secret_version(raw: i32) -> Result<SecretVersion, StoreError> {
    let version_u32 = u32::try_from(raw).unwrap_or(0);
    SecretVersion::try_new(version_u32).ok_or_else(|| {
        StoreError::Database(DbErr::Custom(format!("invalid secret version: {raw}")))
    })
}

// Unique constraint classification

/// Unique constraints for organization-secret writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrganizationSecretConstraint {
    /// Composite index on `(org_id, name)`.
    OrgSecretName,
}

/// Classify unique-constraint failures for organization-secret writes.
#[must_use]
pub(crate) fn classify_org_secret_constraint(err: &DbErr) -> Option<OrganizationSecretConstraint> {
    let Some(sea_orm::SqlErr::UniqueConstraintViolation(details)) = err.sql_err() else {
        return None;
    };
    if details.contains("idx_org_secrets_org_name")
        || details.contains("organization_secrets.org_id, organization_secrets.name")
    {
        return Some(OrganizationSecretConstraint::OrgSecretName);
    }
    None
}

// Store method implementations

/// Create a new organization secret: insert metadata row, encrypt and insert
/// the value version row.
pub(crate) async fn create(
    conn: &DatabaseConnection,
    encryption_key: &SecretEncryptionKey,
    input: CreateOrganizationSecretInput,
) -> Result<CreateOrganizationSecretOutput, CreateOrganizationSecretError> {
    let plaintext = input.secret_value.expose_secret().as_bytes();
    let (ciphertext, nonce) = encryption_key
        .encrypt(plaintext)
        .map_err(CreateOrganizationSecretError::Store)?;

    let secret_id = OrganizationSecretId::fresh();
    let version = SecretVersion::INITIAL;

    let secret_model = entity::organization_secrets::ActiveModel {
        id: Set(secret_id.as_uuid()),
        org_id: Set(input.org_id.as_uuid()),
        name: Set(input.name.as_str().to_owned()),
        owner_scope: Set(input.owner_scope.as_str().to_owned()),
        status: Set(SecretLifecycleStatus::Active.as_str().to_owned()),
        use_policy: Set(input.use_policy.as_str().to_owned()),
        version: Set(i32::try_from(version.value()).unwrap_or(1)),
        description: Set(input.description.clone()),
        provider: Set(input.provider.clone()),
        created_by_account_id: Set(input.creator_account_id.as_uuid()),
        created_at: Set(input.now),
        updated_at: Set(input.now),
        deleted_at: Set(None),
    };

    let inserted_secret = secret_model.insert(conn).await.map_err(|err| {
        if classify_org_secret_constraint(&err).is_some() {
            CreateOrganizationSecretError::DuplicateName
        } else {
            CreateOrganizationSecretError::Store(StoreError::from(err))
        }
    })?;

    let value_model = entity::organization_secret_values::ActiveModel {
        secret_id: Set(inserted_secret.id),
        version: Set(inserted_secret.version),
        encrypted_value: Set(ciphertext),
        nonce: Set(nonce),
        key_id: Set(encryption_key.key_id().to_owned()),
        created_at: Set(input.now),
    };
    value_model
        .insert(conn)
        .await
        .map_err(|err| CreateOrganizationSecretError::Store(StoreError::from(err)))?;

    let record = OrganizationSecretRecord::try_from(inserted_secret)
        .map_err(CreateOrganizationSecretError::Store)?;

    Ok(CreateOrganizationSecretOutput { record })
}

/// Update an existing organization secret: bump version, insert a new
/// encrypted value row, and update the metadata version column.
pub(crate) async fn update(
    conn: &DatabaseConnection,
    encryption_key: &SecretEncryptionKey,
    input: UpdateOrganizationSecretInput,
) -> Result<UpdateOrganizationSecretOutput, UpdateOrganizationSecretError> {
    let existing = entity::organization_secrets::Entity::find_by_id(input.secret_id.as_uuid())
        .one(conn)
        .await
        .map_err(|e| UpdateOrganizationSecretError::Store(StoreError::Database(e)))?;

    let existing = match existing {
        Some(model) => {
            if model.org_id != input.org_id.as_uuid() {
                return Err(UpdateOrganizationSecretError::NotFound);
            }
            if model.deleted_at.is_some() {
                return Err(UpdateOrganizationSecretError::NotFound);
            }
            model
        }
        None => return Err(UpdateOrganizationSecretError::NotFound),
    };

    let current_version =
        parse_db_secret_version(existing.version).map_err(UpdateOrganizationSecretError::Store)?;
    let new_version = current_version
        .next()
        .ok_or_else(|| StoreError::Database(DbErr::Custom("secret version overflow".into())))
        .map_err(UpdateOrganizationSecretError::Store)?;

    let plaintext = input.secret_value.expose_secret().as_bytes();
    let (ciphertext, nonce) = encryption_key
        .encrypt(plaintext)
        .map_err(UpdateOrganizationSecretError::Store)?;

    let value_model = entity::organization_secret_values::ActiveModel {
        secret_id: Set(input.secret_id.as_uuid()),
        version: Set(i32::try_from(new_version.value()).unwrap_or(i32::MAX)),
        encrypted_value: Set(ciphertext),
        nonce: Set(nonce),
        key_id: Set(encryption_key.key_id().to_owned()),
        created_at: Set(input.now),
    };
    value_model
        .insert(conn)
        .await
        .map_err(|e| UpdateOrganizationSecretError::Store(StoreError::Database(e)))?;

    let new_status = input
        .status
        .map_or_else(|| existing.status.clone(), |s| s.as_str().to_owned());
    let new_use_policy = input
        .use_policy
        .map_or_else(|| existing.use_policy.clone(), |u| u.as_str().to_owned());
    let new_description = match input.description {
        Some(desc) => desc,
        None => existing.description.clone(),
    };

    let mut update_model: entity::organization_secrets::ActiveModel = existing.into();
    update_model.version = Set(i32::try_from(new_version.value()).unwrap_or(i32::MAX));
    update_model.status = Set(new_status);
    update_model.use_policy = Set(new_use_policy);
    update_model.description = Set(new_description);
    update_model.updated_at = Set(input.now);

    let updated = update_model
        .update(conn)
        .await
        .map_err(|e| UpdateOrganizationSecretError::Store(StoreError::Database(e)))?;

    let record = OrganizationSecretRecord::try_from(updated)
        .map_err(UpdateOrganizationSecretError::Store)?;

    Ok(UpdateOrganizationSecretOutput { record })
}

/// Soft-delete an organization secret: set `deleted_at` and status to `Retired`.
pub(crate) async fn remove(
    conn: &DatabaseConnection,
    input: RemoveOrganizationSecretInput,
) -> Result<RemoveOrganizationSecretOutput, RemoveOrganizationSecretError> {
    let existing = entity::organization_secrets::Entity::find_by_id(input.secret_id.as_uuid())
        .one(conn)
        .await
        .map_err(|err| RemoveOrganizationSecretError::Store(StoreError::from(err)))?;

    let existing = match existing {
        Some(model) => {
            if model.org_id != input.org_id.as_uuid() {
                return Err(RemoveOrganizationSecretError::NotFound);
            }
            if model.deleted_at.is_some() {
                return Err(RemoveOrganizationSecretError::NotFound);
            }
            model
        }
        None => return Err(RemoveOrganizationSecretError::NotFound),
    };

    let mut update_model: entity::organization_secrets::ActiveModel = existing.into();
    update_model.status = Set(SecretLifecycleStatus::Retired.as_str().to_owned());
    update_model.deleted_at = Set(Some(input.now));
    update_model.updated_at = Set(input.now);

    let updated = update_model
        .update(conn)
        .await
        .map_err(|err| RemoveOrganizationSecretError::Store(StoreError::from(err)))?;

    let record = OrganizationSecretRecord::try_from(updated)
        .map_err(RemoveOrganizationSecretError::Store)?;

    Ok(RemoveOrganizationSecretOutput { record })
}

/// List organization secrets with bounded pagination. Returns metadata only —
/// no encrypted values are touched.
pub(crate) async fn list(
    conn: &DatabaseConnection,
    request: ListOrganizationSecretsRequest,
) -> Result<ListOrganizationSecretsPage, StoreError> {
    let limit = request.limit;
    let cursor = request.cursor;

    let mut query = entity::organization_secrets::Entity::find()
        .filter(entity::organization_secrets::Column::OrgId.eq(request.org_id.as_uuid()))
        .filter(
            entity::organization_secrets::Column::Status.eq(SecretLifecycleStatus::Active.as_str()),
        )
        .order_by_asc(entity::organization_secrets::Column::Id);

    if let Some(after_id) = cursor {
        query = query.filter(entity::organization_secrets::Column::Id.gt(after_id.as_uuid()));
    }

    let paginator = query.paginate(conn, limit);
    let rows = paginator.fetch().await.map_err(StoreError::from)?;

    let mut records = Vec::new();
    let mut last_id = None;
    for row in rows {
        last_id = Some(row.id);
        records.push(OrganizationSecretRecord::try_from(row)?);
    }

    let next_cursor = last_id.map(OrganizationSecretId::new);

    Ok(ListOrganizationSecretsPage {
        records,
        next_cursor,
    })
}

/// Get a single organization secret's metadata by id and org. No value access.
pub(crate) async fn get_metadata(
    conn: &DatabaseConnection,
    org_id: OrgId,
    secret_id: OrganizationSecretId,
) -> Result<Option<OrganizationSecretRecord>, StoreError> {
    let row = entity::organization_secrets::Entity::find_by_id(secret_id.as_uuid())
        .one(conn)
        .await?;

    match row {
        Some(model) => {
            if model.org_id != org_id.as_uuid() || model.deleted_at.is_some() {
                return Ok(None);
            }
            Ok(Some(OrganizationSecretRecord::try_from(model)?))
        }
        None => Ok(None),
    }
}

/// Resolve (decrypt) a secret value for authorized use. This is the only path
/// that touches the encrypted value table. The caller is responsible for
/// checking use policy before calling this method.
pub(crate) async fn resolve_value(
    conn: &DatabaseConnection,
    encryption_key: &SecretEncryptionKey,
    org_id: OrgId,
    secret_id: OrganizationSecretId,
) -> Result<ResolveOrganizationSecretOutput, ResolveOrganizationSecretError> {
    let secret_model = entity::organization_secrets::Entity::find_by_id(secret_id.as_uuid())
        .one(conn)
        .await
        .map_err(|err| ResolveOrganizationSecretError::Store(StoreError::from(err)))?;

    let secret_model = match secret_model {
        Some(model) => {
            if model.org_id != org_id.as_uuid() {
                return Err(ResolveOrganizationSecretError::NotFound);
            }
            if model.deleted_at.is_some() {
                return Err(ResolveOrganizationSecretError::NotFound);
            }
            model
        }
        None => return Err(ResolveOrganizationSecretError::NotFound),
    };

    let use_policy = parse_db_use_policy(&secret_model.use_policy)
        .map_err(ResolveOrganizationSecretError::Store)?;

    let version = parse_db_secret_version(secret_model.version)
        .map_err(ResolveOrganizationSecretError::Store)?;

    let value_row = entity::organization_secret_values::Entity::find_by_id((
        secret_id.as_uuid(),
        i32::try_from(version.value()).unwrap_or(1),
    ))
    .one(conn)
    .await
    .map_err(|err| ResolveOrganizationSecretError::Store(StoreError::from(err)))?;

    let value_row = value_row
        .ok_or_else(|| ResolveOrganizationSecretError::Store(StoreError::SecretStoreUnavailable))?;

    let plaintext_bytes = encryption_key
        .decrypt(&value_row.nonce, &value_row.encrypted_value)
        .map_err(|_| ResolveOrganizationSecretError::Store(StoreError::SecretStoreUnavailable))?;

    let plaintext = String::from_utf8(plaintext_bytes)
        .map_err(|_| ResolveOrganizationSecretError::Store(StoreError::SecretStoreUnavailable))?;

    let record = OrganizationSecretRecord::try_from(secret_model)
        .map_err(ResolveOrganizationSecretError::Store)?;

    Ok(ResolveOrganizationSecretOutput {
        record,
        value: SecretString::from(plaintext),
        use_policy,
    })
}

/// Find an organization secret by org id and name. Used for duplicate-name
/// checks and name-based lookups.
pub(crate) async fn find_by_org_and_name(
    conn: &DatabaseConnection,
    org_id: OrgId,
    name: &OrganizationSecretName,
) -> Result<Option<OrganizationSecretRecord>, OrganizationSecretLookupError> {
    let row = entity::organization_secrets::Entity::find()
        .filter(entity::organization_secrets::Column::OrgId.eq(org_id.as_uuid()))
        .filter(entity::organization_secrets::Column::Name.eq(name.as_str()))
        .filter(entity::organization_secrets::Column::DeletedAt.is_null())
        .one(conn)
        .await
        .map_err(|err| OrganizationSecretLookupError::Store(StoreError::from(err)))?;

    match row {
        Some(model) => Ok(Some(
            OrganizationSecretRecord::try_from(model)
                .map_err(OrganizationSecretLookupError::Store)?,
        )),
        None => Ok(None),
    }
}
