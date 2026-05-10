use tanren_configuration_secrets::{
    CredentialValueSealer, UserCredentialId, UserCredentialSealContext, UserCredentialStatus,
    validate_user_credential_kind, validate_user_credential_value,
};
use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsRequest,
    ListUserCredentialsResponse, RemoveUserCredentialResponse, UpdateUserCredentialRequest,
    UpdateUserCredentialResponse,
};
use tanren_store::{AccountStore, UserConfigurationStore};

use crate::events::{ConfigurationOperation, ConfigurationOperationFailureReason};
use crate::user_configuration_pagination::{
    encode_credentials_cursor, parse_credentials_page_request,
};
use crate::user_configuration_support::{
    AuthenticatedConfigurationContext, ConfigurationOperationRejection,
    append_user_credential_changed_event, append_user_credential_removed_event, ensure_owner_scope,
    item_not_found, map_sealing_error, map_store_error, reject_configuration_operation,
    validation_error,
};
use crate::{AppServiceError, Clock};

const CREDENTIAL_WRITE_STATUS: UserCredentialStatus = UserCredentialStatus::Pending;

pub(crate) async fn add_user_credential<S>(
    store: &S,
    clock: &Clock,
    credential_sealer: &CredentialValueSealer,
    context: AuthenticatedConfigurationContext,
    request: CreateUserCredentialRequest,
) -> Result<CreateUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    if let Err(err) = ensure_owner_scope(context) {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::AddUserCredential,
                reason: ConfigurationOperationFailureReason::ItemNotFound,
                setting_key: None,
                metadata_item_id: None,
            },
            clock.now(),
            err,
        )
        .await;
    }
    if request.owner_scope != context.requested_owner_scope() {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::AddUserCredential,
                reason: ConfigurationOperationFailureReason::ItemNotFound,
                setting_key: None,
                metadata_item_id: None,
            },
            clock.now(),
            item_not_found(),
        )
        .await;
    }
    if let Err(detail) = validate_user_credential_kind(request.kind) {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::AddUserCredential,
                reason: ConfigurationOperationFailureReason::ValidationFailed,
                setting_key: None,
                metadata_item_id: None,
            },
            clock.now(),
            validation_error(detail),
        )
        .await;
    }
    if let Err(detail) = validate_user_credential_value(&request.value) {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::AddUserCredential,
                reason: ConfigurationOperationFailureReason::ValidationFailed,
                setting_key: None,
                metadata_item_id: None,
            },
            clock.now(),
            validation_error(detail),
        )
        .await;
    }

    let now = clock.now();
    let item_id = UserCredentialId::fresh();
    let sealed_value = credential_sealer
        .seal(
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
    credential_sealer: &CredentialValueSealer,
    context: AuthenticatedConfigurationContext,
    item_id: UserCredentialId,
    request: UpdateUserCredentialRequest,
) -> Result<UpdateUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    if let Err(err) = ensure_owner_scope(context) {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::UpdateUserCredential,
                reason: ConfigurationOperationFailureReason::ItemNotFound,
                setting_key: None,
                metadata_item_id: Some(item_id),
            },
            clock.now(),
            err,
        )
        .await;
    }
    if let Err(detail) = validate_user_credential_value(&request.value) {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::UpdateUserCredential,
                reason: ConfigurationOperationFailureReason::ValidationFailed,
                setting_key: None,
                metadata_item_id: Some(item_id),
            },
            clock.now(),
            validation_error(detail),
        )
        .await;
    }

    let Some(existing_item) = store
        .get_user_credential(item_id, context.requested_owner_scope())
        .await
        .map_err(map_store_error)?
    else {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::UpdateUserCredential,
                reason: ConfigurationOperationFailureReason::ItemNotFound,
                setting_key: None,
                metadata_item_id: Some(item_id),
            },
            clock.now(),
            item_not_found(),
        )
        .await;
    };

    let sealed_value = credential_sealer
        .seal(
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
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::UpdateUserCredential,
                reason: ConfigurationOperationFailureReason::ItemNotFound,
                setting_key: None,
                metadata_item_id: Some(item_id),
            },
            clock.now(),
            item_not_found(),
        )
        .await;
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
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::ListUserCredentials,
                reason: ConfigurationOperationFailureReason::ItemNotFound,
                setting_key: None,
                metadata_item_id: None,
            },
            clock.now(),
            err,
        )
        .await;
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
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::RemoveUserCredential,
                reason: ConfigurationOperationFailureReason::ItemNotFound,
                setting_key: None,
                metadata_item_id: Some(item_id),
            },
            clock.now(),
            err,
        )
        .await;
    }

    let removed = store
        .remove_user_credential(item_id, context.requested_owner_scope())
        .await
        .map_err(map_store_error)?;
    let Some(item) = removed else {
        return reject_configuration_operation(
            store,
            context,
            ConfigurationOperationRejection {
                operation: ConfigurationOperation::RemoveUserCredential,
                reason: ConfigurationOperationFailureReason::ItemNotFound,
                setting_key: None,
                metadata_item_id: Some(item_id),
            },
            clock.now(),
            item_not_found(),
        )
        .await;
    };

    let now = clock.now();
    append_user_credential_removed_event(store, context, &item, now).await?;

    Ok(RemoveUserCredentialResponse {
        item: item.into_metadata().into(),
    })
}
