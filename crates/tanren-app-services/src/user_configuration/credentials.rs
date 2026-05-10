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
    AuthenticatedConfigurationContext, append_rejected_event, append_user_credential_changed_event,
    append_user_credential_removed_event, ensure_owner_scope, item_not_found, map_sealing_error,
    map_store_error, validation_error,
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
