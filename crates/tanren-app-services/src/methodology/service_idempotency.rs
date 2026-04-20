use std::future::Future;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tanren_domain::SpecId;

use super::MethodologyService;
use super::errors::{MethodologyError, MethodologyResult, ToolError};

const REQUEST_HASH_ALGO_SHA256_CANONICAL_JSON_V1: &str = "sha256-canonical-json-v1";
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const DEFAULT_IDEMPOTENCY_RESERVATION_LEASE_SECS: i64 = 300;
const MIN_IDEMPOTENCY_RESERVATION_LEASE_SECS: i64 = 30;
const MAX_IDEMPOTENCY_RESERVATION_LEASE_SECS: i64 = 3_600;
const IDEMPOTENCY_STALE_SWEEP_LIMIT: u64 = 128;
const IDEMPOTENCY_RESERVATION_LEASE_ENV: &str = "TANREN_METHODOLOGY_IDEMPOTENCY_LEASE_SECS";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum StoredIdempotencyOutcome {
    Success { response: serde_json::Value },
    Error { error: ToolError },
}

impl MethodologyService {
    pub(crate) async fn run_idempotent_mutation<R, P, F, Fut>(
        &self,
        tool: &str,
        spec_id: SpecId,
        explicit_key: Option<String>,
        payload: &P,
        op: F,
    ) -> MethodologyResult<R>
    where
        R: Serialize + DeserializeOwned,
        P: Serialize,
        F: FnOnce() -> Fut,
        Fut: Future<Output = MethodologyResult<R>>,
    {
        let canonical_payload =
            canonical_json(payload).map_err(|e| MethodologyError::Internal(e.to_string()))?;
        let request_hash = sha256_hex(canonical_payload.as_bytes());
        let derived_key = explicit_key.unwrap_or_else(|| format!("payload:{request_hash}"));
        let scope_key = spec_id.to_string();
        let lease_duration = idempotency_reservation_lease()?;
        loop {
            let now = Utc::now();
            let reservation_expires_at = now + lease_duration;
            let _ = self
                .store
                .purge_expired_methodology_idempotency_reservations(
                    tool,
                    &scope_key,
                    now,
                    IDEMPOTENCY_STALE_SWEEP_LIMIT,
                )
                .await?;

            if let Some(existing) = self
                .store
                .get_methodology_idempotency(tool, &scope_key, &derived_key)
                .await?
            {
                validate_idempotency_hash(tool, &request_hash, &existing)?;
                if let Some(response_json) = existing.response_json {
                    return replay_stored_outcome(&response_json);
                }
                if reservation_is_active(existing.reservation_expires_at, now) {
                    return Err(MethodologyError::Conflict {
                        resource: tool.to_owned(),
                        reason: format!(
                            "idempotency key `{}` is reserved by an unfinished prior attempt",
                            existing.idempotency_key
                        ),
                    });
                }
                let reclaimed = self
                    .store
                    .reclaim_methodology_idempotency_reservation(
                        tool,
                        &scope_key,
                        &derived_key,
                        now,
                        reservation_expires_at,
                    )
                    .await?;
                if reclaimed {
                    break;
                }
                continue;
            }

            let inserted = self
                .store
                .insert_methodology_idempotency_reservation(
                    tanren_store::methodology::InsertMethodologyIdempotencyParams {
                        tool: tool.to_owned(),
                        scope_key: scope_key.clone(),
                        idempotency_key: derived_key.clone(),
                        request_hash: request_hash.clone(),
                        request_hash_algo: REQUEST_HASH_ALGO_SHA256_CANONICAL_JSON_V1.into(),
                        reservation_expires_at,
                    },
                )
                .await?;
            if inserted {
                break;
            }
        }

        match op().await {
            Ok(response) => {
                let response_json = encode_stored_success(&response)?;
                self.store
                    .finalize_methodology_idempotency(
                        tool,
                        &scope_key,
                        &derived_key,
                        response_json,
                        None,
                    )
                    .await?;
                Ok(response)
            }
            Err(err) => {
                let replayable_error = ToolError::from(&err);
                let response_json = encode_stored_error(replayable_error.clone())?;
                self.store
                    .finalize_methodology_idempotency(
                        tool,
                        &scope_key,
                        &derived_key,
                        response_json,
                        None,
                    )
                    .await?;
                Err(MethodologyError::from(replayable_error))
            }
        }
    }
}

fn sha256_hex(input: &[u8]) -> String {
    let digest = Sha256::digest(input);
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push(char::from(HEX_DIGITS[(b >> 4) as usize]));
        out.push(char::from(HEX_DIGITS[(b & 0x0f) as usize]));
    }
    out
}

fn validate_idempotency_hash(
    tool: &str,
    request_hash: &str,
    existing: &tanren_store::methodology::MethodologyIdempotencyEntry,
) -> MethodologyResult<()> {
    if existing.request_hash_algo != REQUEST_HASH_ALGO_SHA256_CANONICAL_JSON_V1
        || existing.request_hash != request_hash
    {
        return Err(MethodologyError::Conflict {
            resource: tool.to_owned(),
            reason: format!(
                "idempotency key `{}` reused with different payload hash",
                existing.idempotency_key
            ),
        });
    }
    Ok(())
}

fn replay_stored_outcome<R: DeserializeOwned>(response_json: &str) -> MethodologyResult<R> {
    let outcome = serde_json::from_str::<StoredIdempotencyOutcome>(response_json)
        .map_err(|e| MethodologyError::Internal(format!("idempotency replay decode: {e}")))?;
    match outcome {
        StoredIdempotencyOutcome::Success { response } => serde_json::from_value::<R>(response)
            .map_err(|e| MethodologyError::Internal(format!("idempotency replay decode: {e}"))),
        StoredIdempotencyOutcome::Error { error } => Err(MethodologyError::from(error)),
    }
}

fn reservation_is_active(
    reservation_expires_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> bool {
    reservation_expires_at.is_some_and(|expires_at| expires_at > now)
}

fn idempotency_reservation_lease() -> MethodologyResult<Duration> {
    let raw = std::env::var(IDEMPOTENCY_RESERVATION_LEASE_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty());
    let seconds = match raw {
        Some(raw) => raw
            .parse::<i64>()
            .map_err(|e| MethodologyError::FieldValidation {
                field_path: format!("/env/{IDEMPOTENCY_RESERVATION_LEASE_ENV}"),
                expected: format!(
                    "integer seconds between {MIN_IDEMPOTENCY_RESERVATION_LEASE_SECS} and {MAX_IDEMPOTENCY_RESERVATION_LEASE_SECS}"
                ),
                actual: format!("{raw} ({e})"),
                remediation:
                    "set TANREN_METHODOLOGY_IDEMPOTENCY_LEASE_SECS to a bounded integer second value"
                        .into(),
            })?,
        None => DEFAULT_IDEMPOTENCY_RESERVATION_LEASE_SECS,
    };
    if !(MIN_IDEMPOTENCY_RESERVATION_LEASE_SECS..=MAX_IDEMPOTENCY_RESERVATION_LEASE_SECS)
        .contains(&seconds)
    {
        return Err(MethodologyError::FieldValidation {
            field_path: format!("/env/{IDEMPOTENCY_RESERVATION_LEASE_ENV}"),
            expected: format!(
                "integer seconds between {MIN_IDEMPOTENCY_RESERVATION_LEASE_SECS} and {MAX_IDEMPOTENCY_RESERVATION_LEASE_SECS}"
            ),
            actual: seconds.to_string(),
            remediation:
                "set TANREN_METHODOLOGY_IDEMPOTENCY_LEASE_SECS to a bounded integer second value"
                    .into(),
        });
    }
    Ok(Duration::seconds(seconds))
}

fn encode_stored_success<R: Serialize>(response: &R) -> MethodologyResult<String> {
    let response_value =
        serde_json::to_value(response).map_err(|e| MethodologyError::Internal(e.to_string()))?;
    serde_json::to_string(&StoredIdempotencyOutcome::Success {
        response: response_value,
    })
    .map_err(|e| MethodologyError::Internal(e.to_string()))
}

fn encode_stored_error(error: ToolError) -> MethodologyResult<String> {
    serde_json::to_string(&StoredIdempotencyOutcome::Error { error })
        .map_err(|e| MethodologyError::Internal(e.to_string()))
}

fn canonical_json<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let raw = serde_json::to_value(value)?;
    let canonical = canonicalize_value(raw);
    serde_json::to_string(&canonical)
}

fn canonicalize_value(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .map(canonicalize_value)
                .collect::<Vec<_>>(),
        ),
        serde_json::Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                if let Some(value) = map.get(&key) {
                    sorted.insert(key, canonicalize_value(value.clone()));
                }
            }
            serde_json::Value::Object(sorted)
        }
        other => other,
    }
}

#[cfg(test)]
#[path = "service_idempotency_tests.rs"]
mod tests;
