//! User-tier configuration and credential persistence adapter.

use argon2::{Algorithm, Argon2, Params, Version};
use async_trait::async_trait;
use chacha20poly1305::{
    ChaCha20Poly1305, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Set, TransactionTrait,
};
use secrecy::{ExposeSecret, SecretString};
use tanren_configuration_secrets::{
    OwnerScope, UserCredentialStatus, UserCredentialWrite, UserSettingKey, UserSettingValue,
    validate_user_credential_value, validate_user_setting,
};
use tanren_identity_policy::AccountId;
use uuid::Uuid;

use crate::{
    Store, StoreError, UserConfigurationStore, UserOwnedItemRecord, UserSettingRecord, entity,
    records::{
        owner_scope_to_db, user_item_kind_to_db, user_item_status_to_db, user_setting_key_to_db,
        user_setting_kind_to_db,
    },
};

const CIPHER_SCHEME_V1: &str = "chacha20poly1305-v1";
const KDF_VERSION_V1: i16 = 1;
const KDF_SALT_LEN_BYTES: usize = 16;
const CREDENTIAL_KEY_LEN_BYTES: usize = 32;
const ARGON2_MEMORY_COST_KIB: u32 = 19_456;
const ARGON2_TIME_COST: u32 = 2;
const ARGON2_PARALLELISM: u32 = 1;
const CREDENTIAL_SEAL_PASSPHRASE_ENV: &str = "TANREN_CREDENTIAL_SEAL_PASSPHRASE";

struct SealedValue {
    kdf_version: i16,
    kdf_salt: [u8; KDF_SALT_LEN_BYTES],
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

#[async_trait]
impl UserConfigurationStore for Store {
    async fn list_user_settings(
        &self,
        account_id: AccountId,
    ) -> Result<Vec<UserSettingRecord>, StoreError> {
        let rows = entity::user_config_values::Entity::find()
            .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
            .order_by_asc(entity::user_config_values::Column::Key)
            .all(&self.conn)
            .await?;
        rows.into_iter().map(UserSettingRecord::try_from).collect()
    }

    async fn get_user_setting(
        &self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> Result<Option<UserSettingRecord>, StoreError> {
        let row = entity::user_config_values::Entity::find()
            .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_config_values::Column::Key.eq(user_setting_key_to_db(key)))
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
        let key_db = user_setting_key_to_db(key);
        if let Some(row) = entity::user_config_values::Entity::find()
            .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_config_values::Column::Key.eq(key_db))
            .one(&self.conn)
            .await?
        {
            let mut active = row.into_active_model();
            active.owner_scope = Set("user".to_owned());
            active.value_kind = Set(user_setting_kind_to_db(&value).to_owned());
            active.value_json = Set(value_json);
            active.updated_at = Set(now);
            return UserSettingRecord::try_from(active.update(&self.conn).await?);
        }

        let inserted = entity::user_config_values::ActiveModel {
            id: Set(Uuid::now_v7()),
            account_id: Set(account_id.as_uuid()),
            owner_scope: Set("user".to_owned()),
            key: Set(key_db.to_owned()),
            value_kind: Set(user_setting_kind_to_db(&value).to_owned()),
            value_json: Set(value_json),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&self.conn)
        .await?;
        UserSettingRecord::try_from(inserted)
    }

    async fn remove_user_setting(
        &self,
        account_id: AccountId,
        key: UserSettingKey,
    ) -> Result<bool, StoreError> {
        let result = entity::user_config_values::Entity::delete_many()
            .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_config_values::Column::Key.eq(user_setting_key_to_db(key)))
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
        let (scope, account_id) = owner_scope_to_db(write.owner_scope);
        let item_id = Uuid::now_v7();
        let sealed = seal_user_value(account_id, item_id, &write.value)?;

        let txn = self.conn.begin().await?;
        let inserted = entity::user_credentials::ActiveModel {
            id: Set(item_id),
            account_id: Set(account_id.as_uuid()),
            owner_scope: Set(scope.to_owned()),
            kind: Set(user_item_kind_to_db(write.kind).to_owned()),
            status: Set(user_item_status_to_db(status).to_owned()),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&txn)
        .await?;

        entity::user_credential_values::ActiveModel {
            id: Set(Uuid::now_v7()),
            item_id: Set(item_id),
            account_id: Set(account_id.as_uuid()),
            cipher_scheme: Set(CIPHER_SCHEME_V1.to_owned()),
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
        id: &str,
        owner_scope: OwnerScope,
        value: SecretString,
        status: UserCredentialStatus,
        now: DateTime<Utc>,
    ) -> Result<Option<UserOwnedItemRecord>, StoreError> {
        validate_user_credential_value(&value).map_err(StoreError::InvalidConfiguration)?;

        let parsed_id = parse_item_id(id)?;
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

        let sealed = seal_user_value(account_id, parsed_id, &value)?;
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
            active_value.cipher_scheme = Set(CIPHER_SCHEME_V1.to_owned());
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
                cipher_scheme: Set(CIPHER_SCHEME_V1.to_owned()),
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
    ) -> Result<Vec<UserOwnedItemRecord>, StoreError> {
        let (scope, account_id) = owner_scope_to_db(owner_scope);
        let rows = entity::user_credentials::Entity::find()
            .filter(entity::user_credentials::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_credentials::Column::OwnerScope.eq(scope))
            .order_by_desc(entity::user_credentials::Column::UpdatedAt)
            .all(&self.conn)
            .await?;
        rows.into_iter()
            .map(UserOwnedItemRecord::try_from)
            .collect()
    }

    async fn remove_user_credential(
        &self,
        id: &str,
        owner_scope: OwnerScope,
    ) -> Result<bool, StoreError> {
        let parsed_id = parse_item_id(id)?;
        let (scope, account_id) = owner_scope_to_db(owner_scope);
        let scoped_credential_ids = entity::user_credentials::Entity::find()
            .select_only()
            .column(entity::user_credentials::Column::Id)
            .filter(entity::user_credentials::Column::Id.eq(parsed_id))
            .filter(entity::user_credentials::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_credentials::Column::OwnerScope.eq(scope))
            .into_query();
        let txn = self.conn.begin().await?;
        entity::user_credential_values::Entity::delete_many()
            .filter(entity::user_credential_values::Column::ItemId.eq(parsed_id))
            .filter(entity::user_credential_values::Column::AccountId.eq(account_id.as_uuid()))
            .filter(
                entity::user_credential_values::Column::ItemId.in_subquery(scoped_credential_ids),
            )
            .exec(&txn)
            .await?;
        let result = entity::user_credentials::Entity::delete_many()
            .filter(entity::user_credentials::Column::Id.eq(parsed_id))
            .filter(entity::user_credentials::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_credentials::Column::OwnerScope.eq(scope))
            .exec(&txn)
            .await?;
        txn.commit().await?;
        Ok(result.rows_affected > 0)
    }
}

fn parse_item_id(value: &str) -> Result<Uuid, StoreError> {
    Uuid::parse_str(value).map_err(|_| StoreError::InvalidStoreValue {
        column: "user_credentials.id",
        detail: value.to_owned(),
    })
}

fn seal_user_value(
    account_id: AccountId,
    item_id: Uuid,
    value: &SecretString,
) -> Result<SealedValue, StoreError> {
    CredentialValueEncryptor::from_env()?.seal(account_id, item_id, value)
}

struct CredentialValueEncryptor {
    passphrase: SecretString,
}

impl CredentialValueEncryptor {
    fn from_env() -> Result<Self, StoreError> {
        let passphrase = std::env::var(CREDENTIAL_SEAL_PASSPHRASE_ENV).map_err(|_| {
            StoreError::CredentialEncryption {
                detail: format!(
                    "missing `{CREDENTIAL_SEAL_PASSPHRASE_ENV}` environment variable for at-rest encryption"
                ),
            }
        })?;
        Ok(Self {
            passphrase: SecretString::new(passphrase.into_boxed_str()),
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
        let key_material = self.derive_key_material(account_id, item_id, kdf_salt)?;
        ChaCha20Poly1305::new_from_slice(&key_material).map_err(|_| {
            StoreError::CredentialEncryption {
                detail: "invalid derived cipher key".to_owned(),
            }
        })
    }

    fn derive_key_material(
        &self,
        account_id: AccountId,
        item_id: Uuid,
        kdf_salt: &[u8; KDF_SALT_LEN_BYTES],
    ) -> Result<[u8; CREDENTIAL_KEY_LEN_BYTES], StoreError> {
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
        let mut output = [0_u8; CREDENTIAL_KEY_LEN_BYTES];
        let mut salt = [0_u8; KDF_SALT_LEN_BYTES + 16 + 16];
        salt[..KDF_SALT_LEN_BYTES].copy_from_slice(kdf_salt);
        salt[KDF_SALT_LEN_BYTES..KDF_SALT_LEN_BYTES + 16]
            .copy_from_slice(account_id.as_uuid().as_bytes());
        salt[KDF_SALT_LEN_BYTES + 16..].copy_from_slice(item_id.as_bytes());
        argon2
            .hash_password_into(
                self.passphrase.expose_secret().as_bytes(),
                &salt,
                &mut output,
            )
            .map_err(|err| StoreError::CredentialEncryption {
                detail: format!("argon2 key derivation failed: {err}"),
            })?;
        Ok(output)
    }
}
