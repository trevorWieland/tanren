//! User-tier configuration and user-owned credential handlers.
//!
//! These handlers enforce authenticated account scope before touching the
//! store layer so cross-account probes return a shared not-found style
//! taxonomy and do not leak existence.

use chrono::Utc;
use secrecy::ExposeSecret;
use tanren_configuration_secrets::{
    ConfigurationValidationFailure, OwnerScope, UserCredentialStatus, UserCredentialWrite,
    UserSettingKey, validate_user_setting,
};
use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsResponse,
    ListUserSettingsResponse, RemoveUserCredentialResponse, RemoveUserSettingResponse,
    UpdateUserCredentialRequest, UpdateUserCredentialResponse, UpsertUserSettingRequest,
    UpsertUserSettingResponse, UserConfigurationFailureReason, UserSettingView,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, StoreError, UserConfigurationStore, UserOwnedItemRecord};

use crate::events::{
    ConfigurationEventType, UserCredentialChanged, UserCredentialRemoved, UserSettingChanged,
    UserSettingRemoved, configuration_envelope,
};
use crate::{AppServiceError, Clock};

const CREATE_STATUS: UserCredentialStatus = UserCredentialStatus::Pending;
const UPDATE_STATUS: UserCredentialStatus = UserCredentialStatus::Pending;

pub(crate) async fn list_user_settings<S>(
    store: &S,
    authenticated_account_id: AccountId,
    requested_account_id: AccountId,
) -> Result<ListUserSettingsResponse, AppServiceError>
where
    S: UserConfigurationStore + ?Sized,
{
    ensure_user_setting_scope(authenticated_account_id, requested_account_id)?;
    let rows = store.list_user_settings(requested_account_id).await?;
    let items = rows
        .into_iter()
        .map(|record| UserSettingView {
            key: record.key,
            value: record.value,
            updated_at: record.updated_at,
        })
        .collect();
    Ok(ListUserSettingsResponse { items })
}

pub(crate) async fn upsert_user_setting<S>(
    store: &S,
    clock: &Clock,
    authenticated_account_id: AccountId,
    requested_account_id: AccountId,
    request: UpsertUserSettingRequest,
) -> Result<UpsertUserSettingResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    ensure_user_setting_scope(authenticated_account_id, requested_account_id)?;
    validate_user_setting(request.key, &request.value).map_err(validation_error)?;

    let now = clock.now();
    let setting = store
        .set_user_setting(
            requested_account_id,
            request.key,
            request.value.clone(),
            now,
        )
        .await
        .map_err(map_store_error)?;

    store
        .append_event(
            configuration_envelope(
                ConfigurationEventType::UserSettingChanged,
                &UserSettingChanged {
                    actor: authenticated_account_id,
                    scope: OwnerScope::User {
                        account_id: requested_account_id,
                    },
                    key: setting.key,
                    value_kind: setting.value.kind(),
                    updated_at: setting.updated_at,
                },
            ),
            now,
        )
        .await?;

    Ok(UpsertUserSettingResponse {
        setting: UserSettingView {
            key: setting.key,
            value: setting.value,
            updated_at: setting.updated_at,
        },
    })
}

pub(crate) async fn remove_user_setting<S>(
    store: &S,
    clock: &Clock,
    authenticated_account_id: AccountId,
    requested_account_id: AccountId,
    key: UserSettingKey,
) -> Result<RemoveUserSettingResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    ensure_user_setting_scope(authenticated_account_id, requested_account_id)?;

    let Some(setting) = store
        .get_user_setting(requested_account_id, key)
        .await
        .map_err(map_store_error)?
    else {
        return Err(setting_not_found());
    };

    let removed = store
        .remove_user_setting(requested_account_id, key)
        .await
        .map_err(map_store_error)?;
    if !removed {
        return Err(setting_not_found());
    }

    let now = clock.now();
    store
        .append_event(
            configuration_envelope(
                ConfigurationEventType::UserSettingRemoved,
                &UserSettingRemoved {
                    actor: authenticated_account_id,
                    scope: OwnerScope::User {
                        account_id: requested_account_id,
                    },
                    key,
                    removed_at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(RemoveUserSettingResponse {
        setting: UserSettingView {
            key: setting.key,
            value: setting.value,
            updated_at: setting.updated_at,
        },
    })
}

pub(crate) async fn add_user_credential<S>(
    store: &S,
    clock: &Clock,
    authenticated_account_id: AccountId,
    request: CreateUserCredentialRequest,
) -> Result<CreateUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    ensure_owner_scope(authenticated_account_id, request.owner_scope)?;

    let write = UserCredentialWrite {
        kind: request.kind,
        owner_scope: request.owner_scope,
        value: request.value,
    };
    write.validate().map_err(validation_error)?;

    let now = clock.now();
    let item = store
        .add_user_credential(write, CREATE_STATUS, now)
        .await
        .map_err(map_store_error)?;

    append_user_credential_changed_event(store, authenticated_account_id, &item, now).await?;

    Ok(CreateUserCredentialResponse {
        item: item.into_metadata().into(),
    })
}

pub(crate) async fn update_user_credential<S>(
    store: &S,
    clock: &Clock,
    authenticated_account_id: AccountId,
    item_id: &str,
    owner_scope: OwnerScope,
    request: UpdateUserCredentialRequest,
) -> Result<UpdateUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    ensure_owner_scope(authenticated_account_id, owner_scope)?;
    if request.value.expose_secret().trim().is_empty() {
        return Err(validation_error(ConfigurationValidationFailure::ValueEmpty));
    }

    let now = clock.now();
    let Some(item) = store
        .update_user_credential(item_id, owner_scope, request.value, UPDATE_STATUS, now)
        .await
        .map_err(map_store_error)?
    else {
        return Err(item_not_found());
    };

    append_user_credential_changed_event(store, authenticated_account_id, &item, now).await?;

    Ok(UpdateUserCredentialResponse {
        item: item.into_metadata().into(),
    })
}

pub(crate) async fn list_user_credentials<S>(
    store: &S,
    authenticated_account_id: AccountId,
    owner_scope: OwnerScope,
) -> Result<ListUserCredentialsResponse, AppServiceError>
where
    S: UserConfigurationStore + ?Sized,
{
    ensure_owner_scope(authenticated_account_id, owner_scope)?;
    let rows = store
        .list_user_credentials(owner_scope)
        .await
        .map_err(map_store_error)?;
    let items = rows
        .into_iter()
        .map(|record| record.into_metadata().into())
        .collect();
    Ok(ListUserCredentialsResponse { items })
}

pub(crate) async fn remove_user_credential<S>(
    store: &S,
    clock: &Clock,
    authenticated_account_id: AccountId,
    item_id: &str,
    owner_scope: OwnerScope,
) -> Result<RemoveUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    ensure_owner_scope(authenticated_account_id, owner_scope)?;

    let item = store
        .list_user_credentials(owner_scope)
        .await
        .map_err(map_store_error)?
        .into_iter()
        .find(|record| record.id == item_id)
        .ok_or_else(item_not_found)?;

    let removed = store
        .remove_user_credential(item_id, owner_scope)
        .await
        .map_err(map_store_error)?;
    if !removed {
        return Err(item_not_found());
    }

    let now = clock.now();
    store
        .append_event(
            configuration_envelope(
                ConfigurationEventType::UserCredentialRemoved,
                &UserCredentialRemoved {
                    actor: authenticated_account_id,
                    scope: owner_scope,
                    item_id: item.id.clone(),
                    kind: item.kind,
                    removed_at: now,
                },
            ),
            now,
        )
        .await?;

    Ok(RemoveUserCredentialResponse {
        item: item.into_metadata().into(),
    })
}

fn ensure_user_setting_scope(
    authenticated_account_id: AccountId,
    requested_account_id: AccountId,
) -> Result<(), AppServiceError> {
    if authenticated_account_id == requested_account_id {
        return Ok(());
    }
    Err(setting_not_found())
}

fn ensure_owner_scope(
    authenticated_account_id: AccountId,
    owner_scope: OwnerScope,
) -> Result<(), AppServiceError> {
    match owner_scope {
        OwnerScope::User { account_id } if account_id == authenticated_account_id => Ok(()),
        OwnerScope::User { .. } => Err(item_not_found()),
    }
}

async fn append_user_credential_changed_event<S>(
    store: &S,
    actor: AccountId,
    item: &UserOwnedItemRecord,
    now: chrono::DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            configuration_envelope(
                ConfigurationEventType::UserCredentialChanged,
                &UserCredentialChanged {
                    actor,
                    scope: item.owner_scope,
                    item_id: item.id.clone(),
                    kind: item.kind,
                    status: item.status,
                    updated_at: item.updated_at,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

fn map_store_error(err: StoreError) -> AppServiceError {
    match err {
        StoreError::InvalidConfiguration(detail) => validation_error(detail),
        other => AppServiceError::Store(other),
    }
}

fn validation_error(detail: ConfigurationValidationFailure) -> AppServiceError {
    AppServiceError::Configuration(UserConfigurationFailureReason::ValidationFailed { detail })
}

fn setting_not_found() -> AppServiceError {
    AppServiceError::Configuration(UserConfigurationFailureReason::SettingNotFound)
}

fn item_not_found() -> AppServiceError {
    AppServiceError::Configuration(UserConfigurationFailureReason::ItemNotFound)
}
