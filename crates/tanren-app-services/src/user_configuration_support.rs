//! Shared support helpers for user-configuration handlers.

use chrono::{DateTime, Utc};
use tanren_configuration_secrets::{
    ConfigurationValidationFailure, OwnerScope, UserCredentialId, UserSettingKey,
};
use tanren_contract::UserConfigurationFailureReason;
use tanren_identity_policy::AccountId;
use tanren_store::{AccountStore, StoreError, UserOwnedItemRecord};

use crate::AppServiceError;
use crate::events::{
    ConfigurationEventType, ConfigurationOperation, ConfigurationOperationFailureReason,
    ConfigurationOperationRejected, CredentialEventRedactionState, UserCredentialChanged,
    UserCredentialRemoved, UserSettingChanged, UserSettingRemoved, configuration_envelope,
};

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

pub(crate) fn ensure_user_setting_scope(
    context: AuthenticatedConfigurationContext,
) -> Result<(), AppServiceError> {
    if context.authenticated_account_id() == context.requested_account_id() {
        return Ok(());
    }
    Err(setting_not_found())
}

pub(crate) fn ensure_owner_scope(
    context: AuthenticatedConfigurationContext,
) -> Result<(), AppServiceError> {
    match context.requested_owner_scope() {
        OwnerScope::User { account_id } if account_id == context.authenticated_account_id() => {
            Ok(())
        }
        OwnerScope::User { .. } => Err(item_not_found()),
    }
}

pub(crate) async fn append_user_setting_changed_event<S>(
    store: &S,
    context: AuthenticatedConfigurationContext,
    key: UserSettingKey,
    value_kind: tanren_configuration_secrets::UserSettingValueKind,
    updated_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            configuration_envelope(
                ConfigurationEventType::UserSettingChanged,
                &UserSettingChanged {
                    actor: context.authenticated_account_id(),
                    scope: OwnerScope::User {
                        account_id: context.requested_account_id(),
                    },
                    key,
                    value_kind,
                    updated_at,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

pub(crate) async fn append_user_setting_removed_event<S>(
    store: &S,
    context: AuthenticatedConfigurationContext,
    key: UserSettingKey,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
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
    Ok(())
}

pub(crate) async fn append_user_credential_changed_event<S>(
    store: &S,
    actor: AccountId,
    item: &UserOwnedItemRecord,
    now: DateTime<Utc>,
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
                    item_id: item.id,
                    kind: item.kind,
                    status: item.status,
                    redaction_state: CredentialEventRedactionState::MetadataOnly,
                    updated_at: item.updated_at,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

pub(crate) async fn append_user_credential_removed_event<S>(
    store: &S,
    context: AuthenticatedConfigurationContext,
    item: &UserOwnedItemRecord,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            configuration_envelope(
                ConfigurationEventType::UserCredentialRemoved,
                &UserCredentialRemoved {
                    actor: context.authenticated_account_id(),
                    scope: context.requested_owner_scope(),
                    item_id: item.id,
                    kind: item.kind,
                    redaction_state: CredentialEventRedactionState::MetadataOnly,
                    removed_at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

pub(crate) async fn append_rejected_event<S>(
    store: &S,
    context: AuthenticatedConfigurationContext,
    operation: ConfigurationOperation,
    reason: ConfigurationOperationFailureReason,
    setting_key: Option<UserSettingKey>,
    metadata_item_id: Option<UserCredentialId>,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            configuration_envelope(
                ConfigurationEventType::ConfigurationOperationRejected,
                &ConfigurationOperationRejected {
                    actor: context.authenticated_account_id(),
                    scope: context.requested_owner_scope(),
                    operation,
                    reason,
                    setting_key,
                    metadata_item_id,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

pub(crate) fn map_store_error(err: StoreError) -> AppServiceError {
    match err {
        StoreError::InvalidConfiguration(detail) => validation_error(detail),
        other => AppServiceError::Store(other),
    }
}

pub(crate) fn map_sealing_error(
    err: &tanren_configuration_secrets::CredentialSealingFailure,
) -> AppServiceError {
    AppServiceError::Store(StoreError::CredentialEncryption {
        detail: err.to_string(),
    })
}

pub(crate) fn validation_error(detail: ConfigurationValidationFailure) -> AppServiceError {
    AppServiceError::Configuration(UserConfigurationFailureReason::ValidationFailed { detail })
}

pub(crate) fn setting_not_found() -> AppServiceError {
    AppServiceError::Configuration(UserConfigurationFailureReason::SettingNotFound)
}

pub(crate) fn item_not_found() -> AppServiceError {
    AppServiceError::Configuration(UserConfigurationFailureReason::ItemNotFound)
}
