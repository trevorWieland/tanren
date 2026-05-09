//! User-tier configuration and credential persistence adapter.
use crate::{
    Store, StoreError, UserConfigurationListPage, UserConfigurationListPageRequest,
    UserConfigurationStore, UserCredentialListCursor, UserOwnedItemRecord, UserSettingListCursor,
    UserSettingRecord, entity,
    records::{
        owner_scope_to_db, user_item_kind_to_db, user_item_status_to_db, user_setting_key_to_db,
        user_setting_kind_to_db,
    },
};
use argon2::{Algorithm, Argon2, Params, Version};
use async_trait::async_trait;
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::OnConflict,
};
use secrecy::{ExposeSecret, SecretString};
use tanren_configuration_secrets::{
    CredentialSealPassphrase, OwnerScope, UserCredentialId, UserCredentialStatus,
    UserCredentialWrite, UserSettingKey, UserSettingValue, validate_user_credential_kind,
    validate_user_credential_value, validate_user_setting,
};
use tanren_identity_policy::AccountId;
use tokio::task;
use uuid::Uuid;
use zeroize::Zeroizing;
const KDF_VERSION_V1: i16 = 1;
const KDF_SALT_LEN_BYTES: usize = 16;
const CREDENTIAL_KEY_LEN_BYTES: usize = 32;
const ARGON2_MEMORY_COST_KIB: u32 = 19_456;
const ARGON2_TIME_COST: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;
const CREDENTIAL_SEAL_PASSPHRASE_ENV: &str = "TANREN_CREDENTIAL_SEAL_PASSPHRASE";
static CREDENTIAL_VALUE_ENCRYPTOR: std::sync::OnceLock<CredentialValueEncryptor> =
    std::sync::OnceLock::new();
struct SealedValue {
    scheme: CredentialSealScheme,
    kdf_version: i16,
    kdf_salt: [u8; KDF_SALT_LEN_BYTES],
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialSealScheme {
    ChaCha20Poly1305V1,
}
impl CredentialSealScheme {
    const fn current() -> Self {
        Self::ChaCha20Poly1305V1
    }
    const fn as_db_value(self) -> &'static str {
        match self {
            Self::ChaCha20Poly1305V1 => "chacha20poly1305-v1",
        }
    }
}
#[async_trait]
impl UserConfigurationStore for Store {
    async fn list_user_settings(
        &self,
        account_id: AccountId,
        page: UserConfigurationListPageRequest<UserSettingListCursor>,
    ) -> Result<UserConfigurationListPage<UserSettingRecord, UserSettingListCursor>, StoreError>
    {
        let limit = usize::from(page.limit);
        let mut query = entity::user_config_values::Entity::find()
            .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
            .order_by_desc(entity::user_config_values::Column::UpdatedAt)
            .order_by_asc(entity::user_config_values::Column::Key);
        if let Some(after) = page.after {
            query = query.filter(
                Condition::any()
                    .add(entity::user_config_values::Column::UpdatedAt.lt(after.updated_at))
                    .add(
                        Condition::all()
                            .add(entity::user_config_values::Column::UpdatedAt.eq(after.updated_at))
                            .add(
                                entity::user_config_values::Column::Key
                                    .gt(user_setting_key_to_db(after.key)?),
                            ),
                    ),
            );
        }
        let rows = query
            .limit(u64::from(page.limit) + 1)
            .all(&self.conn)
            .await?;
        let mut items: Vec<UserSettingRecord> = rows
            .into_iter()
            .map(UserSettingRecord::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = items.len() > limit;
        if has_more {
            let _ = items.pop();
        }
        let next_cursor = if has_more {
            items.last().map(|record| UserSettingListCursor {
                updated_at: record.updated_at,
                key: record.key,
            })
        } else {
            None
        };
        Ok(UserConfigurationListPage { items, next_cursor })
    }
    async fn get_user_setting(
        &self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> Result<Option<UserSettingRecord>, StoreError> {
        let row = entity::user_config_values::Entity::find()
            .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_config_values::Column::Key.eq(user_setting_key_to_db(key)?))
            .one(&self.conn)
            .await?;
        row.map(UserSettingRecord::try_from).transpose()
    }
    async fn set_user_setting(
        &self,
        account_id: AccountId,
        key: UserSettingKey,
        value: UserSettingValue,
        now: DateTime<Utc>,
    ) -> Result<UserSettingRecord, StoreError> {
        validate_user_setting(key, &value)?;
        let value_json = serde_json::to_value(&value).map_err(|source| StoreError::DataEncode {
            field: "user_config_values.value_json",
            source,
        })?;
        let key_db = user_setting_key_to_db(key)?;
        let upserted =
            entity::user_config_values::Entity::insert(entity::user_config_values::ActiveModel {
                id: Set(Uuid::now_v7()),
                account_id: Set(account_id.as_uuid()),
                owner_scope: Set("user".to_owned()),
                key: Set(key_db.to_owned()),
                value_kind: Set(user_setting_kind_to_db(&value).to_owned()),
                value_json: Set(value_json),
                created_at: Set(now),
                updated_at: Set(now),
            })
            .on_conflict(
                OnConflict::columns([
                    entity::user_config_values::Column::AccountId,
                    entity::user_config_values::Column::Key,
                ])
                .update_columns([
                    entity::user_config_values::Column::OwnerScope,
                    entity::user_config_values::Column::ValueKind,
                    entity::user_config_values::Column::ValueJson,
                    entity::user_config_values::Column::UpdatedAt,
                ])
                .to_owned(),
            )
            .exec_with_returning(&self.conn)
            .await?;
        UserSettingRecord::try_from(upserted)
    }
    async fn remove_user_setting(
        &self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> Result<bool, StoreError> {
        let result = entity::user_config_values::Entity::delete_many()
            .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_config_values::Column::Key.eq(user_setting_key_to_db(key)?))
            .exec(&self.conn)
            .await?;
        Ok(result.rows_affected > 0)
    }
    async fn add_user_credential(
        &self,
        write: UserCredentialWrite,
        status: UserCredentialStatus,
        now: DateTime<Utc>,
    ) -> Result<UserOwnedItemRecord, StoreError> {
        write.validate()?;
        let UserCredentialWrite {
            kind,
            owner_scope,
            value,
        } = write;
        validate_user_credential_kind(kind).map_err(StoreError::InvalidConfiguration)?;
        let (scope, account_id) = owner_scope_to_db(owner_scope);
        let item_id = UserCredentialId::fresh();
        let sealed = seal_user_value(account_id, item_id.as_uuid(), value).await?;
        let txn = self.conn.begin().await?;
        let inserted = entity::user_credentials::ActiveModel {
            id: Set(item_id.as_uuid()),
            account_id: Set(account_id.as_uuid()),
            owner_scope: Set(scope.to_owned()),
            kind: Set(user_item_kind_to_db(kind)?.to_owned()),
            status: Set(user_item_status_to_db(status).to_owned()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&txn)
        .await?;
        entity::user_credential_values::ActiveModel {
            id: Set(Uuid::now_v7()),
            item_id: Set(item_id.as_uuid()),
            account_id: Set(account_id.as_uuid()),
            cipher_scheme: Set(sealed.scheme.as_db_value().to_owned()),
            kdf_version: Set(sealed.kdf_version),
            kdf_salt: Set(sealed.kdf_salt.to_vec()),
            nonce: Set(sealed.nonce.to_vec()),
            ciphertext: Set(sealed.ciphertext),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&txn)
        .await?;
        txn.commit().await?;
        UserOwnedItemRecord::try_from(inserted)
    }
    async fn update_user_credential(
        &self,
        id: UserCredentialId,
        owner_scope: OwnerScope,
        value: SecretString,
        status: UserCredentialStatus,
        now: DateTime<Utc>,
    ) -> Result<Option<UserOwnedItemRecord>, StoreError> {
        validate_user_credential_value(&value).map_err(StoreError::InvalidConfiguration)?;
        let parsed_id = id.as_uuid();
        let (scope, account_id) = owner_scope_to_db(owner_scope);
        let row = entity::user_credentials::Entity::find()
            .filter(entity::user_credentials::Column::Id.eq(parsed_id))
            .filter(entity::user_credentials::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_credentials::Column::OwnerScope.eq(scope))
            .one(&self.conn)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let sealed = seal_user_value(account_id, parsed_id, value).await?;
        let txn = self.conn.begin().await?;
        let mut active = row.into_active_model();
        active.status = Set(user_item_status_to_db(status).to_owned());
        active.updated_at = Set(now);
        let updated = active.update(&txn).await?;
        if let Some(existing_value) = entity::user_credential_values::Entity::find()
            .filter(entity::user_credential_values::Column::ItemId.eq(parsed_id))
            .filter(entity::user_credential_values::Column::AccountId.eq(account_id.as_uuid()))
            .one(&txn)
            .await?
        {
            let mut active_value = existing_value.into_active_model();
            active_value.cipher_scheme = Set(sealed.scheme.as_db_value().to_owned());
            active_value.kdf_version = Set(sealed.kdf_version);
            active_value.kdf_salt = Set(sealed.kdf_salt.to_vec());
            active_value.nonce = Set(sealed.nonce.to_vec());
            active_value.ciphertext = Set(sealed.ciphertext);
            active_value.updated_at = Set(now);
            active_value.update(&txn).await?;
        } else {
            entity::user_credential_values::ActiveModel {
                id: Set(Uuid::now_v7()),
                item_id: Set(parsed_id),
                account_id: Set(account_id.as_uuid()),
                cipher_scheme: Set(sealed.scheme.as_db_value().to_owned()),
                kdf_version: Set(sealed.kdf_version),
                kdf_salt: Set(sealed.kdf_salt.to_vec()),
                nonce: Set(sealed.nonce.to_vec()),
                ciphertext: Set(sealed.ciphertext),
                created_at: Set(now),
                updated_at: Set(now),
            }
            .insert(&txn)
            .await?;
        }
        txn.commit().await?;
        Ok(Some(UserOwnedItemRecord::try_from(updated)?))
    }
    async fn list_user_credentials(
        &self,
        owner_scope: OwnerScope,
        page: UserConfigurationListPageRequest<UserCredentialListCursor>,
    ) -> Result<UserConfigurationListPage<UserOwnedItemRecord, UserCredentialListCursor>, StoreError>
    {
        let limit = usize::from(page.limit);
        let (scope, account_id) = owner_scope_to_db(owner_scope);
        let mut query = entity::user_credentials::Entity::find()
            .filter(entity::user_credentials::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_credentials::Column::OwnerScope.eq(scope))
            .order_by_desc(entity::user_credentials::Column::UpdatedAt)
            .order_by_desc(entity::user_credentials::Column::Id);
        if let Some(after) = page.after {
            let after_id = after.id.as_uuid();
            query = query.filter(
                Condition::any()
                    .add(entity::user_credentials::Column::UpdatedAt.lt(after.updated_at))
                    .add(
                        Condition::all()
                            .add(entity::user_credentials::Column::UpdatedAt.eq(after.updated_at))
                            .add(entity::user_credentials::Column::Id.lt(after_id)),
                    ),
            );
        }
        let rows = query
            .limit(u64::from(page.limit) + 1)
            .all(&self.conn)
            .await?;
        let mut items: Vec<UserOwnedItemRecord> = rows
            .into_iter()
            .map(UserOwnedItemRecord::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let has_more = items.len() > limit;
        if has_more {
            let _ = items.pop();
        }
        let next_cursor = if has_more {
            let last = items.last().ok_or_else(|| StoreError::InvalidStoreValue {
                column: "user_credentials.id",
                detail: "credential pagination expected at least one item".to_owned(),
            })?;
            Some(UserCredentialListCursor {
                updated_at: last.updated_at,
                id: last.id,
            })
        } else {
            None
        };
        Ok(UserConfigurationListPage { items, next_cursor })
    }
    async fn get_user_credential(
        &self,
        id: UserCredentialId,
        owner_scope: OwnerScope,
    ) -> Result<Option<UserOwnedItemRecord>, StoreError> {
        let parsed_id = id.as_uuid();
        let (scope, account_id) = owner_scope_to_db(owner_scope);
        let row = entity::user_credentials::Entity::find()
            .filter(entity::user_credentials::Column::Id.eq(parsed_id))
            .filter(entity::user_credentials::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_credentials::Column::OwnerScope.eq(scope))
            .one(&self.conn)
            .await?;
        row.map(UserOwnedItemRecord::try_from).transpose()
    }
    async fn remove_user_credential(
        &self,
        id: UserCredentialId,
        owner_scope: OwnerScope,
    ) -> Result<bool, StoreError> {
        let parsed_id = id.as_uuid();
        let (scope, account_id) = owner_scope_to_db(owner_scope);
        let txn = self.conn.begin().await?;
        let result = entity::user_credentials::Entity::delete_many()
            .filter(entity::user_credentials::Column::Id.eq(parsed_id))
            .filter(entity::user_credentials::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_credentials::Column::OwnerScope.eq(scope))
            .exec(&txn)
            .await?;
        if result.rows_affected == 0 {
            txn.commit().await?;
            return Ok(false);
        }
        entity::user_credential_values::Entity::delete_many()
            .filter(entity::user_credential_values::Column::ItemId.eq(parsed_id))
            .filter(entity::user_credential_values::Column::AccountId.eq(account_id.as_uuid()))
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(true)
    }
}
async fn seal_user_value(
    account_id: AccountId,
    item_id: Uuid,
    value: SecretString,
) -> Result<SealedValue, StoreError> {
    task::spawn_blocking(move || credential_value_encryptor()?.seal(account_id, item_id, &value))
        .await
        .map_err(|_| StoreError::CredentialEncryption {
            detail: "credential seal task failed".to_owned(),
        })?
}
fn credential_value_encryptor() -> Result<&'static CredentialValueEncryptor, StoreError> {
    if let Some(encryptor) = CREDENTIAL_VALUE_ENCRYPTOR.get() {
        return Ok(encryptor);
    }
    let encryptor = CredentialValueEncryptor::from_env()?;
    let _ = CREDENTIAL_VALUE_ENCRYPTOR.set(encryptor);
    CREDENTIAL_VALUE_ENCRYPTOR
        .get()
        .ok_or_else(|| StoreError::CredentialEncryption {
            detail: "failed to initialize credential seal encryptor".to_owned(),
        })
}
struct CredentialValueEncryptor {
    passphrase: CredentialSealPassphrase,
}
impl CredentialValueEncryptor {
    fn from_env() -> Result<Self, StoreError> {
        let passphrase = Zeroizing::new(
            std::env::var(CREDENTIAL_SEAL_PASSPHRASE_ENV).map_err(|_| {
                StoreError::CredentialEncryption {
                    detail: format!(
                        "invalid credential seal configuration in `{CREDENTIAL_SEAL_PASSPHRASE_ENV}`: missing value"
                    ),
                }
            })?,
        );
        let validated =
            CredentialSealPassphrase::parse(passphrase.as_str()).map_err(|validation_error| {
                StoreError::CredentialEncryption {
                    detail: format!(
                        "invalid credential seal configuration in `{CREDENTIAL_SEAL_PASSPHRASE_ENV}`: {validation_error}"
                    ),
                }
            })?;
        Ok(Self {
            passphrase: validated,
        })
    }
    fn seal(
        &self,
        account_id: AccountId,
        item_id: Uuid,
        value: &SecretString,
    ) -> Result<SealedValue, StoreError> {
        let kdf_salt = rand::random::<[u8; KDF_SALT_LEN_BYTES]>();
        let cipher = self.cipher(account_id, item_id, &kdf_salt)?;
        let nonce = rand::random::<[u8; 12]>();
        let account_uuid = account_id.as_uuid();
        let payload = Payload {
            msg: value.expose_secret().as_bytes(),
            aad: account_uuid.as_bytes(),
        };
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), payload)
            .map_err(|_| StoreError::CredentialEncryption {
                detail: "aead encryption failed".to_owned(),
            })?;
        Ok(SealedValue {
            scheme: CredentialSealScheme::current(),
            kdf_version: KDF_VERSION_V1,
            kdf_salt,
            nonce,
            ciphertext,
        })
    }
    fn cipher(
        &self,
        account_id: AccountId,
        item_id: Uuid,
        kdf_salt: &[u8; KDF_SALT_LEN_BYTES],
    ) -> Result<ChaCha20Poly1305, StoreError> {
        let key_material = self.derive_cipher_key(account_id, item_id, kdf_salt)?;
        ChaCha20Poly1305::new_from_slice(&*key_material).map_err(|_| {
            StoreError::CredentialEncryption {
                detail: "invalid derived cipher key".to_owned(),
            }
        })
    }
    fn derive_cipher_key(
        &self,
        account_id: AccountId,
        item_id: Uuid,
        kdf_salt: &[u8; KDF_SALT_LEN_BYTES],
    ) -> Result<Zeroizing<[u8; CREDENTIAL_KEY_LEN_BYTES]>, StoreError> {
        let params = Params::new(
            ARGON2_MEMORY_COST_KIB,
            ARGON2_TIME_COST,
            ARGON2_PARALLELISM,
            Some(CREDENTIAL_KEY_LEN_BYTES),
        )
        .map_err(|err| StoreError::CredentialEncryption {
            detail: format!("invalid argon2 parameters: {err}"),
        })?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut output = Zeroizing::new([0_u8; CREDENTIAL_KEY_LEN_BYTES]);
        let mut salt = Zeroizing::new([0_u8; KDF_SALT_LEN_BYTES + 16 + 16]);
        salt[..KDF_SALT_LEN_BYTES].copy_from_slice(kdf_salt);
        salt[KDF_SALT_LEN_BYTES..KDF_SALT_LEN_BYTES + 16]
            .copy_from_slice(account_id.as_uuid().as_bytes());
        salt[KDF_SALT_LEN_BYTES + 16..].copy_from_slice(item_id.as_bytes());
        argon2
            .hash_password_into(self.passphrase.as_bytes(), &*salt, &mut *output)
            .map_err(|err| StoreError::CredentialEncryption {
                detail: format!("argon2 key derivation failed: {err}"),
            })?;
        Ok(output)
    }
}
