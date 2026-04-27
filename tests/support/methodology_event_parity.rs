use serde_json::Value;
use tanren_domain::events::EventEnvelope;
use uuid::Uuid;

fn is_timestamp_key(key: &str) -> bool {
    matches!(
        key,
        "timestamp" | "created_at" | "updated_at" | "projected_at"
    )
}

fn assert_uuid(value: &str, path: &str) {
    assert!(
        Uuid::parse_str(value).is_ok(),
        "expected UUID at {path}, got `{value}`"
    );
}

fn assert_timestamp(value: &str, path: &str) {
    assert!(
        chrono::DateTime::parse_from_rfc3339(value).is_ok(),
        "expected RFC3339 timestamp at {path}, got `{value}`"
    );
}

fn map_uuid_pair(
    left_uuid: &str,
    right_uuid: &str,
    path: &str,
    id_map: &mut std::collections::BTreeMap<String, String>,
) {
    if let Some(mapped) = id_map.get(left_uuid) {
        assert_eq!(
            mapped, right_uuid,
            "UUID mapping mismatch at {path}: left `{left_uuid}` expected right `{mapped}`, got `{right_uuid}`"
        );
    } else {
        id_map.insert(left_uuid.to_owned(), right_uuid.to_owned());
    }
}

fn is_hex_ascii(b: u8) -> bool {
    b.is_ascii_hexdigit()
}

fn uuid_span_at(bytes: &[u8], start: usize) -> bool {
    const UUID_LEN: usize = 36;
    if start + UUID_LEN > bytes.len() {
        return false;
    }
    let mut idx = start;
    for seg_len in [8usize, 4, 4, 4, 12] {
        for _ in 0..seg_len {
            if !is_hex_ascii(bytes[idx]) {
                return false;
            }
            idx += 1;
        }
        if idx == start + UUID_LEN {
            break;
        }
        if bytes[idx] != b'-' {
            return false;
        }
        idx += 1;
    }
    true
}

fn extract_uuid_spans(value: &str) -> Vec<(usize, usize)> {
    let bytes = value.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0usize;
    while i + 36 <= bytes.len() {
        if uuid_span_at(bytes, i) {
            spans.push((i, i + 36));
            i += 36;
            continue;
        }
        i += 1;
    }
    spans
}

fn assert_string_with_embedded_uuid_parity(
    left: &str,
    right: &str,
    path: &str,
    id_map: &mut std::collections::BTreeMap<String, String>,
) -> bool {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    let left_spans = extract_uuid_spans(left);
    let right_spans = extract_uuid_spans(right);
    if left_spans.is_empty() || left_spans.len() != right_spans.len() {
        return false;
    }

    let mut left_cursor = 0usize;
    let mut right_cursor = 0usize;
    for ((left_start, left_end), (right_start, right_end)) in
        left_spans.iter().zip(right_spans.iter())
    {
        if left_bytes[left_cursor..*left_start] != right_bytes[right_cursor..*right_start] {
            return false;
        }
        let Ok(left_uuid) = std::str::from_utf8(&left_bytes[*left_start..*left_end]) else {
            return false;
        };
        let Ok(right_uuid) = std::str::from_utf8(&right_bytes[*right_start..*right_end]) else {
            return false;
        };
        if Uuid::parse_str(left_uuid).is_err() || Uuid::parse_str(right_uuid).is_err() {
            return false;
        }
        map_uuid_pair(left_uuid, right_uuid, &format!("{path}#embedded"), id_map);
        left_cursor = *left_end;
        right_cursor = *right_end;
    }
    left_bytes[left_cursor..] == right_bytes[right_cursor..]
}

fn assert_value_parity(
    left: &Value,
    right: &Value,
    path: &str,
    id_map: &mut std::collections::BTreeMap<String, String>,
) {
    match (left, right) {
        (Value::Object(left_obj), Value::Object(right_obj)) => {
            let left_keys: std::collections::BTreeSet<&String> = left_obj.keys().collect();
            let right_keys: std::collections::BTreeSet<&String> = right_obj.keys().collect();
            assert_eq!(
                left_keys, right_keys,
                "object key mismatch at {path}: left={left_keys:?} right={right_keys:?}"
            );
            for key in left_obj.keys() {
                let child_path = format!("{path}/{key}");
                let left_value = left_obj.get(key).expect("left key exists");
                let right_value = right_obj.get(key).expect("right key exists");
                if key == "event_id" {
                    assert!(
                        left_value.is_string(),
                        "event_id should be string at {child_path} (left)"
                    );
                    assert!(
                        right_value.is_string(),
                        "event_id should be string at {child_path} (right)"
                    );
                    let left_id = left_value.as_str().unwrap_or_default();
                    let right_id = right_value.as_str().unwrap_or_default();
                    assert_uuid(left_id, &format!("{child_path}(left)"));
                    assert_uuid(right_id, &format!("{child_path}(right)"));
                    map_uuid_pair(left_id, right_id, &child_path, id_map);
                    continue;
                }
                if is_timestamp_key(key) {
                    assert!(
                        left_value.is_string(),
                        "timestamp should be string at {child_path} (left)"
                    );
                    assert!(
                        right_value.is_string(),
                        "timestamp should be string at {child_path} (right)"
                    );
                    let left_ts = left_value.as_str().unwrap_or_default();
                    let right_ts = right_value.as_str().unwrap_or_default();
                    assert_timestamp(left_ts, &format!("{child_path}(left)"));
                    assert_timestamp(right_ts, &format!("{child_path}(right)"));
                    continue;
                }
                assert_value_parity(left_value, right_value, &child_path, id_map);
            }
        }
        (Value::Array(left_items), Value::Array(right_items)) => {
            assert_eq!(
                left_items.len(),
                right_items.len(),
                "array length mismatch at {path}"
            );
            for (idx, (left_item, right_item)) in left_items.iter().zip(right_items).enumerate() {
                assert_value_parity(left_item, right_item, &format!("{path}/{idx}"), id_map);
            }
        }
        (Value::String(left_str), Value::String(right_str)) => {
            let left_uuid = Uuid::parse_str(left_str).ok();
            let right_uuid = Uuid::parse_str(right_str).ok();
            if let (Some(_), Some(_)) = (left_uuid, right_uuid) {
                map_uuid_pair(left_str, right_str, path, id_map);
            } else {
                if assert_string_with_embedded_uuid_parity(left_str, right_str, path, id_map) {
                    return;
                }
                assert_eq!(
                    left_str, right_str,
                    "string mismatch at {path}: left=`{left_str}` right=`{right_str}`"
                );
            }
        }
        _ => assert_eq!(left, right, "value mismatch at {path}"),
    }
}

pub(crate) fn assert_event_stream_strict_parity(
    cli_events: &[EventEnvelope],
    mcp_events: &[EventEnvelope],
) {
    assert_eq!(
        cli_events.len(),
        mcp_events.len(),
        "event stream length mismatch"
    );
    let mut id_map = std::collections::BTreeMap::new();
    for (idx, (cli, mcp)) in cli_events.iter().zip(mcp_events).enumerate() {
        let cli_value = serde_json::to_value(cli).expect("serialize CLI envelope");
        let mcp_value = serde_json::to_value(mcp).expect("serialize MCP envelope");
        assert_value_parity(
            &cli_value,
            &mcp_value,
            &format!("/event_stream/{idx}"),
            &mut id_map,
        );
    }
}

pub(crate) fn assert_phase_lines_strict_parity(cli_lines: &[Value], mcp_lines: &[Value]) {
    assert_eq!(
        cli_lines.len(),
        mcp_lines.len(),
        "phase-events.jsonl line count mismatch"
    );
    let mut id_map = std::collections::BTreeMap::new();
    for (idx, (cli, mcp)) in cli_lines.iter().zip(mcp_lines).enumerate() {
        assert_value_parity(cli, mcp, &format!("/phase_lines/{idx}"), &mut id_map);
    }
}
