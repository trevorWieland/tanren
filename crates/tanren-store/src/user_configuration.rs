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
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, EntityTrait, IntoActiveModel, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::OnConflict,
};
use tanren_configuration_secrets::{
    OwnerScope, SealedUserCredentialValue, UserCredentialId, UserCredentialStatus, UserSettingKey,
    UserSettingValue, validate_user_credential_kind, validate_user_setting,
};
use tanren_identity_policy::AccountId;
use uuid::Uuid;

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
        sealed_value: SealedUserCredentialValue,
        status: UserCredentialStatus,
        now: DateTime<Utc>,
    ) -> Result<UserOwnedItemRecord, StoreError> {
        let item_id = sealed_value.credential_id();
        let kind = sealed_value.credential_kind();
        let owner_scope = sealed_value.owner_scope();
        validate_user_credential_kind(kind).map_err(StoreError::InvalidConfiguration)?;
        let (scope, account_id) = owner_scope_to_db(owner_scope);
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
            cipher_scheme: Set(sealed_value.scheme().as_db_value().to_owned()),
            kdf_version: Set(sealed_value.kdf_version()),
            kdf_salt: Set(sealed_value.kdf_salt().to_vec()),
            nonce: Set(sealed_value.nonce().to_vec()),
            ciphertext: Set(sealed_value.ciphertext().to_vec()),
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
        sealed_value: SealedUserCredentialValue,
        status: UserCredentialStatus,
        now: DateTime<Utc>,
    ) -> Result<Option<UserOwnedItemRecord>, StoreError> {
        let parsed_id = id.as_uuid();
        let (scope, account_id) = owner_scope_to_db(owner_scope);
        if sealed_value.credential_id() != id {
            return Err(StoreError::CredentialEncryption {
                detail: "sealed credential id does not match update target".to_owned(),
            });
        }
        if sealed_value.owner_scope() != owner_scope {
            return Err(StoreError::CredentialEncryption {
                detail: "sealed owner scope does not match update target".to_owned(),
            });
        }
        let row = entity::user_credentials::Entity::find()
            .filter(entity::user_credentials::Column::Id.eq(parsed_id))
            .filter(entity::user_credentials::Column::AccountId.eq(account_id.as_uuid()))
            .filter(entity::user_credentials::Column::OwnerScope.eq(scope))
            .one(&self.conn)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let row_kind = crate::records::parse_user_item_kind(&row.kind)?;
        if row_kind != sealed_value.credential_kind() {
            return Err(StoreError::CredentialEncryption {
                detail: "sealed credential kind does not match persisted metadata kind".to_owned(),
            });
        }
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
            active_value.cipher_scheme = Set(sealed_value.scheme().as_db_value().to_owned());
            active_value.kdf_version = Set(sealed_value.kdf_version());
            active_value.kdf_salt = Set(sealed_value.kdf_salt().to_vec());
            active_value.nonce = Set(sealed_value.nonce().to_vec());
            active_value.ciphertext = Set(sealed_value.ciphertext().to_vec());
            active_value.updated_at = Set(now);
            active_value.update(&txn).await?;
        } else {
            entity::user_credential_values::ActiveModel {
                id: Set(Uuid::now_v7()),
                item_id: Set(parsed_id),
                account_id: Set(account_id.as_uuid()),
                cipher_scheme: Set(sealed_value.scheme().as_db_value().to_owned()),
                kdf_version: Set(sealed_value.kdf_version()),
                kdf_salt: Set(sealed_value.kdf_salt().to_vec()),
                nonce: Set(sealed_value.nonce().to_vec()),
                ciphertext: Set(sealed_value.ciphertext().to_vec()),
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
        txn.commit().await?;
        Ok(true)
    }
}
