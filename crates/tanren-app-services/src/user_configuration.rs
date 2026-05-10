use tanren_configuration_secrets::{
    UserCredentialId, UserCredentialSealContext, UserCredentialStatus, UserSettingKey,
    seal_user_credential_value_from_env, validate_user_credential_kind,
    validate_user_credential_value, validate_user_setting,
};
use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsRequest,
    ListUserCredentialsResponse, ListUserSettingsRequest, ListUserSettingsResponse,
    RemoveUserCredentialResponse, RemoveUserSettingResponse, UpdateUserCredentialRequest,
    UpdateUserCredentialResponse, UpsertUserSettingRequest, UpsertUserSettingResponse,
    UserSettingView,
};
use tanren_store::{AccountStore, UserConfigurationStore};

use crate::events::{ConfigurationOperation, ConfigurationOperationFailureReason};
use crate::user_configuration_pagination::{
    encode_credentials_cursor, encode_settings_cursor, parse_credentials_page_request,
    parse_settings_page_request,
};
use crate::user_configuration_support::{
    AuthenticatedConfigurationContext, append_rejected_event, append_user_credential_changed_event,
    append_user_credential_removed_event, append_user_setting_changed_event,
    append_user_setting_removed_event, ensure_owner_scope, ensure_user_setting_scope,
    item_not_found, map_sealing_error, map_store_error, setting_not_found, validation_error,
};
use crate::{AppServiceError, Clock};

const CREDENTIAL_WRITE_STATUS: UserCredentialStatus = UserCredentialStatus::Pending;

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
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::ListUserSettings,
            ConfigurationOperationFailureReason::SettingNotFound,
            None,
            None,
            clock.now(),
        )
        .await?;
        return Err(err);
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
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::UpsertUserSetting,
            ConfigurationOperationFailureReason::SettingNotFound,
            Some(request.key),
            None,
            clock.now(),
        )
        .await?;
        return Err(err);
    }
    if let Err(detail) = validate_user_setting(request.key, &request.value) {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::UpsertUserSetting,
            ConfigurationOperationFailureReason::ValidationFailed,
            Some(request.key),
            None,
            clock.now(),
        )
        .await?;
        return Err(validation_error(detail));
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
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::RemoveUserSetting,
            ConfigurationOperationFailureReason::SettingNotFound,
            Some(key),
            None,
            clock.now(),
        )
        .await?;
        return Err(err);
    }

    let Some(setting) = store
        .get_user_setting(context.requested_account_id(), key)
        .await
        .map_err(map_store_error)?
    else {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::RemoveUserSetting,
            ConfigurationOperationFailureReason::SettingNotFound,
            Some(key),
            None,
            clock.now(),
        )
        .await?;
        return Err(setting_not_found());
    };

    let removed = store
        .remove_user_setting(context.requested_account_id(), key)
        .await
        .map_err(map_store_error)?;
    if !removed {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::RemoveUserSetting,
            ConfigurationOperationFailureReason::SettingNotFound,
            Some(key),
            None,
            clock.now(),
        )
        .await?;
        return Err(setting_not_found());
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

pub(crate) async fn add_user_credential<S>(
    store: &S,
    clock: &Clock,
    context: AuthenticatedConfigurationContext,
    request: CreateUserCredentialRequest,
) -> Result<CreateUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    if let Err(err) = ensure_owner_scope(context) {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::AddUserCredential,
            ConfigurationOperationFailureReason::ItemNotFound,
            None,
            None,
            clock.now(),
        )
        .await?;
        return Err(err);
    }
    if request.owner_scope != context.requested_owner_scope() {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::AddUserCredential,
            ConfigurationOperationFailureReason::ItemNotFound,
            None,
            None,
            clock.now(),
        )
        .await?;
        return Err(item_not_found());
    }
    if let Err(detail) = validate_user_credential_kind(request.kind) {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::AddUserCredential,
            ConfigurationOperationFailureReason::ValidationFailed,
            None,
            None,
            clock.now(),
        )
        .await?;
        return Err(validation_error(detail));
    }
    if let Err(detail) = validate_user_credential_value(&request.value) {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::AddUserCredential,
            ConfigurationOperationFailureReason::ValidationFailed,
            None,
            None,
            clock.now(),
        )
        .await?;
        return Err(validation_error(detail));
    }

    let now = clock.now();
    let item_id = UserCredentialId::fresh();
    let sealed_value = seal_user_credential_value_from_env(
        UserCredentialSealContext {
            credential_id: item_id,
            owner_scope: context.requested_owner_scope(),
            credential_kind: request.kind,
        },
        request.value,
    )
    .await
    .map_err(|err| map_sealing_error(&err))?;

    let item = store
        .add_user_credential(sealed_value, CREDENTIAL_WRITE_STATUS, now)
        .await
        .map_err(map_store_error)?;

    append_user_credential_changed_event(store, context.authenticated_account_id(), &item, now)
        .await?;

    Ok(CreateUserCredentialResponse {
        item: item.into_metadata().into(),
    })
}

pub(crate) async fn update_user_credential<S>(
    store: &S,
    clock: &Clock,
    context: AuthenticatedConfigurationContext,
    item_id: UserCredentialId,
    request: UpdateUserCredentialRequest,
) -> Result<UpdateUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    if let Err(err) = ensure_owner_scope(context) {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::UpdateUserCredential,
            ConfigurationOperationFailureReason::ItemNotFound,
            None,
            Some(item_id),
            clock.now(),
        )
        .await?;
        return Err(err);
    }
    if let Err(detail) = validate_user_credential_value(&request.value) {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::UpdateUserCredential,
            ConfigurationOperationFailureReason::ValidationFailed,
            None,
            Some(item_id),
            clock.now(),
        )
        .await?;
        return Err(validation_error(detail));
    }

    let Some(existing_item) = store
        .get_user_credential(item_id, context.requested_owner_scope())
        .await
        .map_err(map_store_error)?
    else {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::UpdateUserCredential,
            ConfigurationOperationFailureReason::ItemNotFound,
            None,
            Some(item_id),
            clock.now(),
        )
        .await?;
        return Err(item_not_found());
    };

    let sealed_value = seal_user_credential_value_from_env(
        UserCredentialSealContext {
            credential_id: item_id,
            owner_scope: context.requested_owner_scope(),
            credential_kind: existing_item.kind,
        },
        request.value,
    )
    .await
    .map_err(|err| map_sealing_error(&err))?;

    let now = clock.now();
    let Some(item) = store
        .update_user_credential(
            item_id,
            context.requested_owner_scope(),
            sealed_value,
            CREDENTIAL_WRITE_STATUS,
            now,
        )
        .await
        .map_err(map_store_error)?
    else {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::UpdateUserCredential,
            ConfigurationOperationFailureReason::ItemNotFound,
            None,
            Some(item_id),
            clock.now(),
        )
        .await?;
        return Err(item_not_found());
    };

    append_user_credential_changed_event(store, context.authenticated_account_id(), &item, now)
        .await?;

    Ok(UpdateUserCredentialResponse {
        item: item.into_metadata().into(),
    })
}

pub(crate) async fn list_user_credentials<S>(
    store: &S,
    clock: &Clock,
    context: AuthenticatedConfigurationContext,
    request: ListUserCredentialsRequest,
) -> Result<ListUserCredentialsResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    if let Err(err) = ensure_owner_scope(context) {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::ListUserCredentials,
            ConfigurationOperationFailureReason::ItemNotFound,
            None,
            None,
            clock.now(),
        )
        .await?;
        return Err(err);
    }
    let page = parse_credentials_page_request(request)?;
    let rows = store
        .list_user_credentials(context.requested_owner_scope(), page)
        .await
        .map_err(map_store_error)?;
    let items = rows
        .items
        .into_iter()
        .map(|record| record.into_metadata().into())
        .collect();
    Ok(ListUserCredentialsResponse {
        items,
        next_cursor: rows.next_cursor.as_ref().map(encode_credentials_cursor),
    })
}

pub(crate) async fn remove_user_credential<S>(
    store: &S,
    clock: &Clock,
    context: AuthenticatedConfigurationContext,
    item_id: UserCredentialId,
) -> Result<RemoveUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    if let Err(err) = ensure_owner_scope(context) {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::RemoveUserCredential,
            ConfigurationOperationFailureReason::ItemNotFound,
            None,
            Some(item_id),
            clock.now(),
        )
        .await?;
        return Err(err);
    }

    let Some(item) = store
        .get_user_credential(item_id, context.requested_owner_scope())
        .await
        .map_err(map_store_error)?
    else {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::RemoveUserCredential,
            ConfigurationOperationFailureReason::ItemNotFound,
            None,
            Some(item_id),
            clock.now(),
        )
        .await?;
        return Err(item_not_found());
    };

    let removed = store
        .remove_user_credential(item_id, context.requested_owner_scope())
        .await
        .map_err(map_store_error)?;
    if !removed {
        append_rejected_event(
            store,
            context,
            ConfigurationOperation::RemoveUserCredential,
            ConfigurationOperationFailureReason::ItemNotFound,
            None,
            Some(item_id),
            clock.now(),
        )
        .await?;
        return Err(item_not_found());
    }

    let now = clock.now();
    append_user_credential_removed_event(store, context, &item, now).await?;

    Ok(RemoveUserCredentialResponse {
        item: item.into_metadata().into(),
    })
}
