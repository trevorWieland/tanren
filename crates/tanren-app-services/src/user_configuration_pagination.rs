use base64::Engine;
use tanren_contract::{
    ListUserCredentialsRequest, ListUserSettingsRequest, UserCredentialsPageCursorKind,
    UserCredentialsPageCursorPayload, UserSettingsPageCursorKind, UserSettingsPageCursorPayload,
};
use tanren_store::{
    UserConfigurationListPageRequest, UserCredentialListCursor, UserSettingListCursor,
};

use crate::AppServiceError;

pub(super) const DEFAULT_LIST_LIMIT: u16 = 50;
const MAX_LIST_LIMIT: u16 = 100;

pub(super) fn parse_settings_page_request(
    request: ListUserSettingsRequest,
) -> Result<UserConfigurationListPageRequest<UserSettingListCursor>, AppServiceError> {
    let limit = parse_limit(request.limit)?;
    let after = match request.after {
        Some(cursor) => Some(decode_settings_cursor(&cursor)?),
        None => None,
    };
    Ok(UserConfigurationListPageRequest { limit, after })
}

pub(super) fn parse_credentials_page_request(
    request: ListUserCredentialsRequest,
) -> Result<UserConfigurationListPageRequest<UserCredentialListCursor>, AppServiceError> {
    let limit = parse_limit(request.limit)?;
    let after = match request.after {
        Some(cursor) => Some(decode_credentials_cursor(&cursor)?),
        None => None,
    };
    Ok(UserConfigurationListPageRequest { limit, after })
}

pub(super) fn encode_settings_cursor(cursor: UserSettingListCursor) -> String {
    encode_cursor_payload(&UserSettingsPageCursorPayload {
        kind: UserSettingsPageCursorKind::Settings,
        updated_at: cursor.updated_at,
        key: cursor.key,
    })
}

pub(super) fn encode_credentials_cursor(cursor: &UserCredentialListCursor) -> String {
    encode_cursor_payload(&UserCredentialsPageCursorPayload {
        kind: UserCredentialsPageCursorKind::Credentials,
        updated_at: cursor.updated_at,
        id: cursor.id,
    })
}

fn parse_limit(limit: Option<u16>) -> Result<u16, AppServiceError> {
    let effective = limit.unwrap_or(DEFAULT_LIST_LIMIT);
    if effective == 0 || effective > MAX_LIST_LIMIT {
        return Err(AppServiceError::InvalidInput(format!(
            "limit must be between 1 and {MAX_LIST_LIMIT}"
        )));
    }
    Ok(effective)
}

fn decode_settings_cursor(raw: &str) -> Result<UserSettingListCursor, AppServiceError> {
    let payload: UserSettingsPageCursorPayload = decode_cursor_payload(raw)?;
    Ok(UserSettingListCursor {
        updated_at: payload.updated_at,
        key: payload.key,
    })
}

fn decode_credentials_cursor(raw: &str) -> Result<UserCredentialListCursor, AppServiceError> {
    let payload: UserCredentialsPageCursorPayload = decode_cursor_payload(raw)?;
    Ok(UserCredentialListCursor {
        updated_at: payload.updated_at,
        id: payload.id,
    })
}

fn encode_cursor_payload(payload: &impl serde::Serialize) -> String {
    match serde_json::to_vec(payload) {
        Ok(bytes) => base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes),
        Err(_) => String::new(),
    }
}

fn decode_cursor_payload<T>(raw: &str) -> Result<T, AppServiceError>
where
    T: serde::de::DeserializeOwned,
{
    if raw.trim().is_empty() {
        return Err(malformed_after_cursor());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| malformed_after_cursor())?;
    serde_json::from_slice::<T>(&bytes).map_err(|_| malformed_after_cursor())
}

fn malformed_after_cursor() -> AppServiceError {
    AppServiceError::InvalidInput("after cursor is malformed".to_owned())
}
