use chrono::{DateTime, Utc};
use secrecy::ExposeSecret;
use tanren_configuration_secrets::{
    CredentialId, CredentialKind, CredentialScope, RedactedCredentialMetadata, UserSettingKey,
    UserSettingValue,
};
use tanren_contract::{
    ConfigurationFailureReason, CreateCredentialRequest, CreateCredentialResponse,
    GetUserConfigRequest, GetUserConfigResponse, ListCredentialsResponse, ListUserConfigResponse,
    RemoveCredentialRequest, RemoveCredentialResponse, RemoveUserConfigRequest,
    RemoveUserConfigResponse, SetUserConfigRequest, SetUserConfigResponse, UpdateCredentialRequest,
    UpdateCredentialResponse, UserConfigEntry,
};
use tanren_identity_policy::AccountId;
use tanren_store::{
    AccountStore, CredentialRecord, NewCredential, NewUserConfigValue, StoreError,
    UpdateCredential, UserConfigRecord,
};

use crate::events::{
    CREDENTIAL_ADD_REJECTED_KIND, CREDENTIAL_REMOVE_REJECTED_KIND, CREDENTIAL_UPDATE_REJECTED_KIND,
    CredentialAddRejected, CredentialRemoveRejected, CredentialUpdateRejected,
    USER_CONFIG_SET_REJECTED_KIND, UserConfigSetRejected, configuration_envelope,
};
use crate::{AppServiceError, AuthenticatedActor, Clock};

pub(crate) async fn list_user_config<S>(
    store: &S,
    actor: &AuthenticatedActor,
) -> Result<ListUserConfigResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let records = store.list_user_config(actor.account_id()).await?;
    Ok(ListUserConfigResponse {
        entries: records.into_iter().map(config_entry).collect(),
    })
}

pub(crate) async fn get_user_config<S>(
    store: &S,
    actor: &AuthenticatedActor,
    request: GetUserConfigRequest,
) -> Result<GetUserConfigResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let record = store
        .get_user_config(actor.account_id(), request.key)
        .await?;
    Ok(GetUserConfigResponse {
        entry: record.map(config_entry),
    })
}

pub(crate) async fn set_user_config<S>(
    store: &S,
    clock: &Clock,
    actor: &AuthenticatedActor,
    request: SetUserConfigRequest,
) -> Result<SetUserConfigResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let account_id = actor.account_id();

    if UserSettingValue::parse(request.value.as_str()).is_err() {
        emit_config_set_rejected(
            store,
            account_id,
            request.key,
            ConfigurationFailureReason::InvalidSettingValue,
            now,
        )
        .await?;
        return Err(AppServiceError::Configuration(
            ConfigurationFailureReason::InvalidSettingValue,
        ));
    }

    let record = store
        .set_user_config(NewUserConfigValue {
            account_id,
            key: request.key,
            value: request.value,
            now,
        })
        .await?;

    Ok(SetUserConfigResponse {
        entry: config_entry(record),
    })
}

pub(crate) async fn remove_user_config<S>(
    store: &S,
    clock: &Clock,
    actor: &AuthenticatedActor,
    request: RemoveUserConfigRequest,
) -> Result<RemoveUserConfigResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let removed = store
        .remove_user_config(actor.account_id(), request.key, now)
        .await?;
    Ok(RemoveUserConfigResponse { removed })
}

pub(crate) async fn list_credentials<S>(
    store: &S,
    actor: &AuthenticatedActor,
) -> Result<ListCredentialsResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let records = store.list_credentials(actor.account_id()).await?;
    Ok(ListCredentialsResponse {
        credentials: records.iter().map(redacted_credential).collect(),
    })
}

pub(crate) async fn create_credential<S>(
    store: &S,
    clock: &Clock,
    actor: &AuthenticatedActor,
    request: CreateCredentialRequest,
) -> Result<CreateCredentialResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let account_id = actor.account_id();

    let name = request.name.trim().to_owned();
    if name.is_empty() || request.value.expose_secret().is_empty() {
        emit_credential_add_rejected(
            store,
            account_id,
            &request.name,
            request.kind,
            ConfigurationFailureReason::ValidationFailed,
            now,
        )
        .await?;
        return Err(AppServiceError::Configuration(
            ConfigurationFailureReason::ValidationFailed,
        ));
    }

    let existing = store.list_credentials(account_id).await?;
    if existing
        .iter()
        .any(|c| c.name == name && c.kind == request.kind)
    {
        emit_credential_add_rejected(
            store,
            account_id,
            &name,
            request.kind,
            ConfigurationFailureReason::DuplicateCredentialName,
            now,
        )
        .await?;
        return Err(AppServiceError::Configuration(
            ConfigurationFailureReason::DuplicateCredentialName,
        ));
    }

    let encrypted_value = request.value.expose_secret().as_bytes().to_vec();

    let record = store
        .add_credential(NewCredential {
            account_id,
            kind: request.kind,
            scope: CredentialScope::User,
            name,
            description: request.description,
            provider: request.provider,
            encrypted_value,
            now,
        })
        .await
        .map_err(map_credential_insert_error)?;

    Ok(CreateCredentialResponse {
        credential: redacted_credential(&record),
    })
}

pub(crate) async fn update_credential<S>(
    store: &S,
    clock: &Clock,
    actor: &AuthenticatedActor,
    request: UpdateCredentialRequest,
) -> Result<UpdateCredentialResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let account_id = actor.account_id();

    let existing =
        store
            .get_credential(request.id)
            .await?
            .ok_or(AppServiceError::Configuration(
                ConfigurationFailureReason::CredentialNotFound,
            ))?;

    if existing.account_id != account_id {
        emit_credential_update_rejected(
            store,
            request.id,
            account_id,
            ConfigurationFailureReason::Unauthorized,
            now,
        )
        .await?;
        return Err(AppServiceError::Configuration(
            ConfigurationFailureReason::Unauthorized,
        ));
    }

    if request.value.expose_secret().is_empty() {
        emit_credential_update_rejected(
            store,
            request.id,
            account_id,
            ConfigurationFailureReason::ValidationFailed,
            now,
        )
        .await?;
        return Err(AppServiceError::Configuration(
            ConfigurationFailureReason::ValidationFailed,
        ));
    }

    let encrypted_value = request.value.expose_secret().as_bytes().to_vec();

    let record = store
        .update_credential(UpdateCredential {
            id: request.id,
            name: request.name,
            description: request.description,
            encrypted_value,
            now,
        })
        .await?
        .ok_or(AppServiceError::Configuration(
            ConfigurationFailureReason::CredentialNotFound,
        ))?;

    Ok(UpdateCredentialResponse {
        credential: redacted_credential(&record),
    })
}

pub(crate) async fn remove_credential<S>(
    store: &S,
    clock: &Clock,
    actor: &AuthenticatedActor,
    request: RemoveCredentialRequest,
) -> Result<RemoveCredentialResponse, AppServiceError>
where
    S: AccountStore + ?Sized,
{
    let now = clock.now();
    let account_id = actor.account_id();

    let existing = store.get_credential(request.id).await?;
    match existing {
        None => {
            return Err(AppServiceError::Configuration(
                ConfigurationFailureReason::CredentialNotFound,
            ));
        }
        Some(rec) if rec.account_id != account_id => {
            emit_credential_remove_rejected(
                store,
                request.id,
                account_id,
                ConfigurationFailureReason::Unauthorized,
                now,
            )
            .await?;
            return Err(AppServiceError::Configuration(
                ConfigurationFailureReason::Unauthorized,
            ));
        }
        Some(_) => {}
    }

    let removed = store.remove_credential(request.id, now).await?;
    Ok(RemoveCredentialResponse { removed })
}

fn config_entry(record: UserConfigRecord) -> UserConfigEntry {
    UserConfigEntry {
        key: record.key,
        value: record.value,
        updated_at: record.updated_at,
    }
}

fn redacted_credential(record: &CredentialRecord) -> RedactedCredentialMetadata {
    RedactedCredentialMetadata {
        id: record.id,
        name: record.name.clone(),
        kind: record.kind,
        scope: record.scope,
        description: record.description.clone(),
        provider: record.provider.clone(),
        created_at: record.created_at,
        updated_at: record.updated_at,
        present: record.present,
    }
}

async fn emit_config_set_rejected<S>(
    store: &S,
    account_id: AccountId,
    key: UserSettingKey,
    reason: ConfigurationFailureReason,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            configuration_envelope(
                USER_CONFIG_SET_REJECTED_KIND,
                &UserConfigSetRejected {
                    account_id,
                    key,
                    reason,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

async fn emit_credential_add_rejected<S>(
    store: &S,
    account_id: AccountId,
    name: &str,
    kind: CredentialKind,
    reason: ConfigurationFailureReason,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            configuration_envelope(
                CREDENTIAL_ADD_REJECTED_KIND,
                &CredentialAddRejected {
                    account_id,
                    name: name.to_owned(),
                    kind,
                    reason,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

async fn emit_credential_update_rejected<S>(
    store: &S,
    id: CredentialId,
    account_id: AccountId,
    reason: ConfigurationFailureReason,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            configuration_envelope(
                CREDENTIAL_UPDATE_REJECTED_KIND,
                &CredentialUpdateRejected {
                    id,
                    account_id,
                    reason,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

async fn emit_credential_remove_rejected<S>(
    store: &S,
    id: CredentialId,
    account_id: AccountId,
    reason: ConfigurationFailureReason,
    now: DateTime<Utc>,
) -> Result<(), AppServiceError>
where
    S: AccountStore + ?Sized,
{
    store
        .append_event(
            configuration_envelope(
                CREDENTIAL_REMOVE_REJECTED_KIND,
                &CredentialRemoveRejected {
                    id,
                    account_id,
                    reason,
                    at: now,
                },
            ),
            now,
        )
        .await?;
    Ok(())
}

fn map_credential_insert_error(err: StoreError) -> AppServiceError {
    let message = err.to_string().to_lowercase();
    if message.contains("unique") || message.contains("duplicate") {
        AppServiceError::Configuration(ConfigurationFailureReason::DuplicateCredentialName)
    } else {
        AppServiceError::Store(err)
    }
}
