//! `SeaORM`-backed implementations of user-configuration and credential
//! CRUD operations, plus session-token-to-account lookup. Extracted into
//! its own module so `lib.rs` stays under the workspace per-file line
//! budget.
//!
//! Lifecycle mutations (`set`, `remove`, `add`, `update`) append a
//! metadata-only event to the canonical event log. Event payloads never
//! contain secret material or encrypted values.

use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tanren_configuration_secrets::{CredentialId, CredentialKind, CredentialScope, UserSettingKey};
use tanren_identity_policy::AccountId;
use uuid::Uuid;

use crate::entity;
use crate::{
    CredentialRecord, NewCredential, NewUserConfigValue, StoreError, UpdateCredential,
    UserConfigRecord,
};

async fn append_lifecycle_event(
    conn: &DatabaseConnection,
    payload: serde_json::Value,
    now: DateTime<Utc>,
) -> Result<(), StoreError> {
    let model = entity::events::ActiveModel {
        id: Set(Uuid::now_v7()),
        occurred_at: Set(now),
        payload: Set(payload),
    };
    model.insert(conn).await.map_err(StoreError::from)?;
    Ok(())
}

pub(crate) async fn list_user_config(
    conn: &DatabaseConnection,
    account_id: AccountId,
) -> Result<Vec<UserConfigRecord>, StoreError> {
    let rows = entity::user_config_values::Entity::find()
        .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
        .all(conn)
        .await?;
    rows.into_iter().map(UserConfigRecord::try_from).collect()
}

pub(crate) async fn get_user_config(
    conn: &DatabaseConnection,
    account_id: AccountId,
    key: UserSettingKey,
) -> Result<Option<UserConfigRecord>, StoreError> {
    let key_str = setting_key_to_db(key);
    let row = entity::user_config_values::Entity::find()
        .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::user_config_values::Column::Key.eq(key_str))
        .one(conn)
        .await?;
    row.map(UserConfigRecord::try_from).transpose()
}

pub(crate) async fn set_user_config(
    conn: &DatabaseConnection,
    input: NewUserConfigValue,
) -> Result<UserConfigRecord, StoreError> {
    let key_str = setting_key_to_db(input.key);
    let existing = entity::user_config_values::Entity::find()
        .filter(entity::user_config_values::Column::AccountId.eq(input.account_id.as_uuid()))
        .filter(entity::user_config_values::Column::Key.eq(&key_str))
        .one(conn)
        .await?;

    let record = if let Some(model) = existing {
        let mut active: entity::user_config_values::ActiveModel = model.into();
        active.value = Set(input.value.as_str().to_owned());
        active.updated_at = Set(input.now);
        let updated = active.update(conn).await?;
        UserConfigRecord::try_from(updated)?
    } else {
        let model = entity::user_config_values::ActiveModel {
            id: Set(Uuid::now_v7()),
            account_id: Set(input.account_id.as_uuid()),
            key: Set(key_str),
            value: Set(input.value.as_str().to_owned()),
            updated_at: Set(input.now),
        };
        let inserted = model.insert(conn).await?;
        UserConfigRecord::try_from(inserted)?
    };

    let payload = serde_json::json!({
        "type": "user_config_set",
        "account_id": record.account_id.as_uuid().to_string(),
        "key": setting_key_to_db(record.key),
        "updated_at": record.updated_at.to_rfc3339(),
    });
    append_lifecycle_event(conn, payload, record.updated_at).await?;
    Ok(record)
}

pub(crate) async fn remove_user_config(
    conn: &DatabaseConnection,
    account_id: AccountId,
    key: UserSettingKey,
    now: DateTime<Utc>,
) -> Result<bool, StoreError> {
    let key_str = setting_key_to_db(key);
    let existing = entity::user_config_values::Entity::find()
        .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::user_config_values::Column::Key.eq(&key_str))
        .one(conn)
        .await?;
    let result = entity::user_config_values::Entity::delete_many()
        .filter(entity::user_config_values::Column::AccountId.eq(account_id.as_uuid()))
        .filter(entity::user_config_values::Column::Key.eq(key_str))
        .exec(conn)
        .await?;
    if result.rows_affected > 0 {
        let mut payload = serde_json::json!({
            "type": "user_config_removed",
            "account_id": account_id.as_uuid().to_string(),
            "key": setting_key_to_db(key),
        });
        if let Some(model) = existing {
            let rec = UserConfigRecord::try_from(model)?;
            payload["updated_at"] = serde_json::Value::String(rec.updated_at.to_rfc3339());
        }
        append_lifecycle_event(conn, payload, now).await?;
    }
    Ok(result.rows_affected > 0)
}

pub(crate) async fn list_credentials(
    conn: &DatabaseConnection,
    account_id: AccountId,
) -> Result<Vec<CredentialRecord>, StoreError> {
    let rows = entity::user_credentials::Entity::find()
        .filter(entity::user_credentials::Column::AccountId.eq(account_id.as_uuid()))
        .all(conn)
        .await?;
    rows.into_iter().map(CredentialRecord::try_from).collect()
}

pub(crate) async fn get_credential(
    conn: &DatabaseConnection,
    id: CredentialId,
) -> Result<Option<CredentialRecord>, StoreError> {
    let row = entity::user_credentials::Entity::find_by_id(id.as_uuid())
        .one(conn)
        .await?;
    row.map(CredentialRecord::try_from).transpose()
}

pub(crate) async fn add_credential(
    conn: &DatabaseConnection,
    input: NewCredential,
) -> Result<CredentialRecord, StoreError> {
    let kind_str = credential_kind_to_db(input.kind);
    let scope_str = credential_scope_to_db(input.scope);
    let model = entity::user_credentials::ActiveModel {
        id: Set(Uuid::now_v7()),
        account_id: Set(input.account_id.as_uuid()),
        kind: Set(kind_str),
        scope: Set(scope_str),
        name: Set(input.name),
        description: Set(input.description),
        provider: Set(input.provider),
        encrypted_value: Set(input.encrypted_value),
        created_at: Set(input.now),
        updated_at: Set(None),
    };
    let inserted = model.insert(conn).await?;
    let record = CredentialRecord::try_from(inserted)?;

    let payload = serde_json::json!({
        "type": "credential_added",
        "id": record.id.as_uuid().to_string(),
        "account_id": record.account_id.as_uuid().to_string(),
        "kind": credential_kind_to_db(record.kind),
        "scope": credential_scope_to_db(record.scope),
        "name": record.name,
        "provider": record.provider,
        "created_at": record.created_at.to_rfc3339(),
    });
    append_lifecycle_event(conn, payload, record.created_at).await?;
    Ok(record)
}

pub(crate) async fn update_credential(
    conn: &DatabaseConnection,
    input: UpdateCredential,
) -> Result<Option<CredentialRecord>, StoreError> {
    let existing = entity::user_credentials::Entity::find_by_id(input.id.as_uuid())
        .one(conn)
        .await?;
    let Some(model) = existing else {
        return Ok(None);
    };
    let mut active: entity::user_credentials::ActiveModel = model.into();
    if let Some(name) = input.name {
        active.name = Set(name);
    }
    if let Some(description) = input.description {
        active.description = Set(Some(description));
    }
    active.encrypted_value = Set(input.encrypted_value);
    active.updated_at = Set(Some(input.now));
    let updated = active.update(conn).await?;
    let record = CredentialRecord::try_from(updated)?;

    let payload = serde_json::json!({
        "type": "credential_updated",
        "id": record.id.as_uuid().to_string(),
        "account_id": record.account_id.as_uuid().to_string(),
        "updated_at": input.now.to_rfc3339(),
    });
    append_lifecycle_event(conn, payload, input.now).await?;
    Ok(Some(record))
}

pub(crate) async fn remove_credential(
    conn: &DatabaseConnection,
    id: CredentialId,
    now: DateTime<Utc>,
) -> Result<bool, StoreError> {
    let existing = entity::user_credentials::Entity::find_by_id(id.as_uuid())
        .one(conn)
        .await?;
    let result = entity::user_credentials::Entity::delete_by_id(id.as_uuid())
        .exec(conn)
        .await?;
    if result.rows_affected > 0 {
        let mut payload = serde_json::json!({
            "type": "credential_removed",
            "id": id.as_uuid().to_string(),
        });
        if let Some(model) = existing {
            let rec = CredentialRecord::try_from(model)?;
            payload["account_id"] = serde_json::Value::String(rec.account_id.as_uuid().to_string());
            payload["kind"] = serde_json::Value::String(credential_kind_to_db(rec.kind));
            payload["scope"] = serde_json::Value::String(credential_scope_to_db(rec.scope));
            payload["name"] = serde_json::Value::String(rec.name);
            if let Some(provider) = rec.provider {
                payload["provider"] = serde_json::Value::String(provider);
            }
        }
        append_lifecycle_event(conn, payload, now).await?;
    }
    Ok(result.rows_affected > 0)
}

pub(crate) async fn find_account_id_by_session_token(
    conn: &DatabaseConnection,
    token: &str,
    now: DateTime<Utc>,
) -> Result<Option<AccountId>, StoreError> {
    let row = entity::account_sessions::Entity::find()
        .filter(entity::account_sessions::Column::Token.eq(token))
        .filter(entity::account_sessions::Column::ExpiresAt.gt(now))
        .one(conn)
        .await?;
    Ok(row.map(|r| AccountId::new(r.account_id)))
}

pub(crate) fn setting_key_to_db(key: UserSettingKey) -> String {
    match key {
        UserSettingKey::PreferredHarness => "preferred_harness".to_owned(),
        UserSettingKey::PreferredProvider => "preferred_provider".to_owned(),
        _ => "unknown".to_owned(),
    }
}

pub(crate) fn setting_key_from_db(raw: &str) -> Result<UserSettingKey, StoreError> {
    match raw {
        "preferred_harness" => Ok(UserSettingKey::PreferredHarness),
        "preferred_provider" => Ok(UserSettingKey::PreferredProvider),
        _ => Err(StoreError::Deserialization {
            entity: "user_config_values",
            column: "key",
            cause: format!("unknown user setting key: {raw}"),
        }),
    }
}

pub(crate) fn credential_kind_to_db(kind: CredentialKind) -> String {
    match kind {
        CredentialKind::ApiKey => "api_key".to_owned(),
        CredentialKind::SourceControlToken => "source_control_token".to_owned(),
        CredentialKind::WebhookSigningKey => "webhook_signing_key".to_owned(),
        CredentialKind::OidcClientSecret => "oidc_client_secret".to_owned(),
        CredentialKind::OpaqueSecret => "opaque_secret".to_owned(),
        _ => "unknown".to_owned(),
    }
}

pub(crate) fn credential_kind_from_db(raw: &str) -> Result<CredentialKind, StoreError> {
    match raw {
        "api_key" => Ok(CredentialKind::ApiKey),
        "source_control_token" => Ok(CredentialKind::SourceControlToken),
        "webhook_signing_key" => Ok(CredentialKind::WebhookSigningKey),
        "oidc_client_secret" => Ok(CredentialKind::OidcClientSecret),
        "opaque_secret" => Ok(CredentialKind::OpaqueSecret),
        _ => Err(StoreError::Deserialization {
            entity: "user_credentials",
            column: "kind",
            cause: format!("unknown credential kind: {raw}"),
        }),
    }
}

pub(crate) fn credential_scope_to_db(scope: CredentialScope) -> String {
    match scope {
        CredentialScope::User => "user".to_owned(),
        CredentialScope::Project => "project".to_owned(),
        CredentialScope::Organization => "organization".to_owned(),
        CredentialScope::ServiceAccount => "service_account".to_owned(),
        _ => "unknown".to_owned(),
    }
}

pub(crate) fn credential_scope_from_db(raw: &str) -> Result<CredentialScope, StoreError> {
    match raw {
        "user" => Ok(CredentialScope::User),
        "project" => Ok(CredentialScope::Project),
        "organization" => Ok(CredentialScope::Organization),
        "service_account" => Ok(CredentialScope::ServiceAccount),
        _ => Err(StoreError::Deserialization {
            entity: "user_credentials",
            column: "scope",
            cause: format!("unknown credential scope: {raw}"),
        }),
    }
}
