//! User-tier configuration and user-owned credential handlers.
//!
//! These handlers enforce authenticated account scope before touching the
//! store layer so cross-account probes return a shared not-found style
//! taxonomy and do not leak existence.

use chrono::Utc;
use tanren_configuration_secrets::{
    ConfigurationValidationFailure, OwnerScope, UserCredentialStatus, UserCredentialWrite,
    UserSettingKey, validate_user_credential_value, validate_user_setting,
};
use tanren_contract::{
    CreateUserCredentialRequest, CreateUserCredentialResponse, ListUserCredentialsRequest,
    ListUserCredentialsResponse, ListUserSettingsRequest, ListUserSettingsResponse,
    RemoveUserCredentialResponse, RemoveUserSettingResponse, UpdateUserCredentialRequest,
    UpdateUserCredentialResponse, UpsertUserSettingRequest, UpsertUserSettingResponse,
    UserConfigurationFailureReason, UserSettingView,
};
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, StoreError, UserConfigurationStore, UserOwnedItemRecord};

use crate::events::{
    ConfigurationEventType, UserCredentialChanged, UserCredentialRemoved, UserSettingChanged,
    UserSettingRemoved, configuration_envelope,
};
use crate::user_configuration_pagination::{
    encode_credentials_cursor, encode_settings_cursor, parse_credentials_page_request,
    parse_settings_page_request,
};
use crate::{AppServiceError, Clock};

const CREATE_STATUS: UserCredentialStatus = UserCredentialStatus::Pending;
const UPDATE_STATUS: UserCredentialStatus = UserCredentialStatus::Pending;

/// Shared context for authenticated user-configuration operations.
///
/// The authenticated actor and requested scope are carried separately so
/// handlers can enforce same-account rules without conflating the two values.
#[derive(Debug, Clone, Copy)]
pub struct AuthenticatedConfigurationContext {
    authenticated_account_id: AccountId,
    requested_account_id: AccountId,
    requested_owner_scope: OwnerScope,
}

impl AuthenticatedConfigurationContext {
    /// Build context for a request targeting a specific account id.
    #[must_use]
    pub const fn for_requested_account(
        authenticated_account_id: AccountId,
        requested_account_id: AccountId,
    ) -> Self {
        Self {
            authenticated_account_id,
            requested_account_id,
            requested_owner_scope: OwnerScope::User {
                account_id: requested_account_id,
            },
        }
    }

    /// Build context for a request targeting an explicit owner scope.
    #[must_use]
    pub const fn for_requested_owner_scope(
        authenticated_account_id: AccountId,
        requested_owner_scope: OwnerScope,
    ) -> Self {
        let requested_account_id = match requested_owner_scope {
            OwnerScope::User { account_id } => account_id,
        };
        Self {
            authenticated_account_id,
            requested_account_id,
            requested_owner_scope,
        }
    }

    #[must_use]
    pub const fn authenticated_account_id(self) -> AccountId {
        self.authenticated_account_id
    }

    #[must_use]
    pub const fn requested_account_id(self) -> AccountId {
        self.requested_account_id
    }

    #[must_use]
    pub const fn requested_owner_scope(self) -> OwnerScope {
        self.requested_owner_scope
    }
}

pub(crate) async fn list_user_settings<S>(
    store: &S,
    context: AuthenticatedConfigurationContext,
    request: ListUserSettingsRequest,
) -> Result<ListUserSettingsResponse, AppServiceError>
where
    S: UserConfigurationStore + ?Sized,
{
    ensure_user_setting_scope(context)?;
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
    ensure_user_setting_scope(context)?;
    validate_user_setting(request.key, &request.value).map_err(validation_error)?;

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

    store
        .append_event(
            configuration_envelope(
                ConfigurationEventType::UserSettingChanged,
                &UserSettingChanged {
                    actor: context.authenticated_account_id(),
                    scope: OwnerScope::User {
                        account_id: context.requested_account_id(),
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
    context: AuthenticatedConfigurationContext,
    key: UserSettingKey,
) -> Result<RemoveUserSettingResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    ensure_user_setting_scope(context)?;

    let Some(setting) = store
        .get_user_setting(context.requested_account_id(), key)
        .await
        .map_err(map_store_error)?
    else {
        return Err(setting_not_found());
    };

    let removed = store
        .remove_user_setting(context.requested_account_id(), key)
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
                    actor: context.authenticated_account_id(),
                    scope: OwnerScope::User {
                        account_id: context.requested_account_id(),
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
    context: AuthenticatedConfigurationContext,
    request: CreateUserCredentialRequest,
) -> Result<CreateUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    ensure_owner_scope(context)?;
    if request.owner_scope != context.requested_owner_scope() {
        return Err(item_not_found());
    }

    let write = UserCredentialWrite {
        kind: request.kind,
        owner_scope: context.requested_owner_scope(),
        value: request.value,
    };
    write.validate().map_err(validation_error)?;

    let now = clock.now();
    let item = store
        .add_user_credential(write, CREATE_STATUS, now)
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
    item_id: &str,
    request: UpdateUserCredentialRequest,
) -> Result<UpdateUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    ensure_owner_scope(context)?;
    validate_user_credential_value(&request.value).map_err(validation_error)?;

    let now = clock.now();
    let Some(item) = store
        .update_user_credential(
            item_id,
            context.requested_owner_scope(),
            request.value,
            UPDATE_STATUS,
            now,
        )
        .await
        .map_err(map_store_error)?
    else {
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
    context: AuthenticatedConfigurationContext,
    request: ListUserCredentialsRequest,
) -> Result<ListUserCredentialsResponse, AppServiceError>
where
    S: UserConfigurationStore + ?Sized,
{
    ensure_owner_scope(context)?;
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
    item_id: &str,
) -> Result<RemoveUserCredentialResponse, AppServiceError>
where
    S: UserConfigurationStore + AccountStore + ?Sized,
{
    ensure_owner_scope(context)?;

    let item = store
        .get_user_credential(item_id, context.requested_owner_scope())
        .await
        .map_err(map_store_error)?
        .ok_or_else(item_not_found)?;

    let removed = store
        .remove_user_credential(item_id, context.requested_owner_scope())
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
                    actor: context.authenticated_account_id(),
                    scope: context.requested_owner_scope(),
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
    context: AuthenticatedConfigurationContext,
) -> Result<(), AppServiceError> {
    if context.authenticated_account_id() == context.requested_account_id() {
        return Ok(());
    }
    Err(setting_not_found())
}

fn ensure_owner_scope(context: AuthenticatedConfigurationContext) -> Result<(), AppServiceError> {
    match context.requested_owner_scope() {
        OwnerScope::User { account_id } if account_id == context.authenticated_account_id() => {
            Ok(())
        }
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
