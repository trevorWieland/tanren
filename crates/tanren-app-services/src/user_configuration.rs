mod credentials;

pub(crate) use credentials::{
    add_user_credential, list_user_credentials, remove_user_credential, update_user_credential,
};

use tanren_configuration_secrets::{UserSettingKey, validate_user_setting};
use tanren_contract::{
    ListUserSettingsRequest, ListUserSettingsResponse, RemoveUserSettingResponse,
    UpsertUserSettingRequest, UpsertUserSettingResponse, UserSettingView,
};
use tanren_store::{AccountStore, UserConfigurationStore};

use crate::events::{ConfigurationOperation, ConfigurationOperationFailureReason};
use crate::user_configuration_pagination::{encode_settings_cursor, parse_settings_page_request};
use crate::user_configuration_support::{
    AuthenticatedConfigurationContext, ConfigurationOperationRejection,
    append_user_setting_changed_event, append_user_setting_removed_event,
    ensure_user_setting_scope, map_store_error, reject_configuration_operation, setting_not_found,
    validation_error,
};
use crate::{AppServiceError, Clock};

pub(crate) async fn list_user_settings<S>(
    store: &S,
    clock: &Clock,
    context: AuthenticatedConfigurationContext,
    request: ListUserSettingsRequest,
) -> Result<ListUserSettingsResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    if let Err(err) = ensure_user_setting_scope(context) {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::ListUserSettings,
                reason: ConfigurationOperationFailureReason::SettingNotFound,
                setting_key: None,
                metadata_item_id: None,
            },
            clock.now(),
            err,
        )
        .await;
    }
    let page = parse_settings_page_request(request)?;
    let rows = store
        .list_user_settings(context.requested_account_id(), page)
        .await
        .map_err(map_store_error)?;
    let items = rows
        .items
        .into_iter()
        .map(|record| UserSettingView {
            key: record.key,
            value: record.value,
            updated_at: record.updated_at,
        })
        .collect();
    Ok(ListUserSettingsResponse {
        items,
        next_cursor: rows.next_cursor.map(encode_settings_cursor),
    })
}

pub(crate) async fn upsert_user_setting<S>(
    store: &S,
    clock: &Clock,
    context: AuthenticatedConfigurationContext,
    request: UpsertUserSettingRequest,
) -> Result<UpsertUserSettingResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    if let Err(err) = ensure_user_setting_scope(context) {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::UpsertUserSetting,
                reason: ConfigurationOperationFailureReason::SettingNotFound,
                setting_key: Some(request.key),
                metadata_item_id: None,
            },
            clock.now(),
            err,
        )
        .await;
    }
    if let Err(detail) = validate_user_setting(request.key, &request.value) {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::UpsertUserSetting,
                reason: ConfigurationOperationFailureReason::ValidationFailed,
                setting_key: Some(request.key),
                metadata_item_id: None,
            },
            clock.now(),
            validation_error(detail),
        )
        .await;
    }

    let now = clock.now();
    let setting = store
        .set_user_setting(
            context.requested_account_id(),
            request.key,
            request.value.clone(),
            now,
        )
        .await
        .map_err(map_store_error)?;

    append_user_setting_changed_event(
        store,
        context,
        setting.key,
        setting.value.kind(),
        setting.updated_at,
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
    context: AuthenticatedConfigurationContext,
    key: UserSettingKey,
) -> Result<RemoveUserSettingResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    if let Err(err) = ensure_user_setting_scope(context) {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::RemoveUserSetting,
                reason: ConfigurationOperationFailureReason::SettingNotFound,
                setting_key: Some(key),
                metadata_item_id: None,
            },
            clock.now(),
            err,
        )
        .await;
    }

    let Some(setting) = store
        .get_user_setting(context.requested_account_id(), key)
        .await
        .map_err(map_store_error)?
    else {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::RemoveUserSetting,
                reason: ConfigurationOperationFailureReason::SettingNotFound,
                setting_key: Some(key),
                metadata_item_id: None,
            },
            clock.now(),
            setting_not_found(),
        )
        .await;
    };

    let removed = store
        .remove_user_setting(context.requested_account_id(), key)
        .await
        .map_err(map_store_error)?;
    if !removed {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::RemoveUserSetting,
                reason: ConfigurationOperationFailureReason::SettingNotFound,
                setting_key: Some(key),
                metadata_item_id: None,
            },
            clock.now(),
            setting_not_found(),
        )
        .await;
    }

    let now = clock.now();
    append_user_setting_removed_event(store, context, key, now).await?;

    Ok(RemoveUserSettingResponse {
        setting: UserSettingView {
            key: setting.key,
            value: setting.value,
            updated_at: setting.updated_at,
        },
    })
}
