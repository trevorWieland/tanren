use base64::Engine;
use chrono::Utc;
use tanren_configuration_secrets::UserSettingKey;
use tanren_contract::{ListUserCredentialsRequest, ListUserSettingsRequest};
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
    encode_cursor_payload(&format!(
        "{}|{}",
        cursor.updated_at.to_rfc3339(),
        setting_key_name(cursor.key)
    ))
}

pub(super) fn encode_credentials_cursor(cursor: &UserCredentialListCursor) -> String {
    encode_cursor_payload(&format!("{}|{}", cursor.updated_at.to_rfc3339(), cursor.id))
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
    let payload = decode_cursor_payload(raw)?;
    let mut parts = payload.splitn(2, '|');
    let updated_at_raw = parts.next().ok_or_else(malformed_after_cursor)?;
    let key_raw = parts.next().ok_or_else(malformed_after_cursor)?;
    let parsed = chrono::DateTime::parse_from_rfc3339(updated_at_raw)
        .map_err(|_| malformed_after_cursor())?;
    let key = parse_setting_key(key_raw).ok_or_else(malformed_after_cursor)?;
    Ok(UserSettingListCursor {
        updated_at: parsed.with_timezone(&Utc),
        key,
    })
}

fn decode_credentials_cursor(raw: &str) -> Result<UserCredentialListCursor, AppServiceError> {
    let payload = decode_cursor_payload(raw)?;
    let mut parts = payload.splitn(2, '|');
    let updated_at_raw = parts.next().ok_or_else(malformed_after_cursor)?;
    let id = parts.next().ok_or_else(malformed_after_cursor)?;
    let parsed = chrono::DateTime::parse_from_rfc3339(updated_at_raw)
        .map_err(|_| malformed_after_cursor())?;
    if id.trim().is_empty() {
        return Err(malformed_after_cursor());
    }
    Ok(UserCredentialListCursor {
        updated_at: parsed.with_timezone(&Utc),
        id: id.to_owned(),
    })
}

fn encode_cursor_payload(payload: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload)
}

fn decode_cursor_payload(raw: &str) -> Result<String, AppServiceError> {
    if raw.trim().is_empty() {
        return Err(malformed_after_cursor());
    }
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw)
        .map_err(|_| malformed_after_cursor())?;
    String::from_utf8(bytes).map_err(|_| malformed_after_cursor())
}

fn malformed_after_cursor() -> AppServiceError {
    AppServiceError::InvalidInput("after cursor is malformed".to_owned())
}

fn setting_key_name(key: UserSettingKey) -> &'static str {
    match key {
        UserSettingKey::Theme => "theme",
        UserSettingKey::Editor => "editor",
    }
}

fn parse_setting_key(raw: &str) -> Option<UserSettingKey> {
    match raw {
        "theme" => Some(UserSettingKey::Theme),
        "editor" => Some(UserSettingKey::Editor),
        _ => None,
    }
}
