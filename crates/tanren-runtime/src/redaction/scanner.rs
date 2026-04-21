pub(crate) fn find_ascii_case_insensitive(
    haystack: &str,
    needle: &str,
    start: usize,
) -> Option<usize> {
    if needle.is_empty() || start >= haystack.len() {
        return None;
    }

    let haystack_bytes = haystack.as_bytes();
    let needle_bytes = needle.as_bytes();
    if haystack_bytes.len() < needle_bytes.len() {
        return None;
    }

    let mut idx = start;
    while idx + needle_bytes.len() <= haystack_bytes.len() {
        let mut match_all = true;
        for offset in 0..needle_bytes.len() {
            if !haystack_bytes[idx + offset].eq_ignore_ascii_case(&needle_bytes[offset]) {
                match_all = false;
                break;
            }
        }
        if match_all {
            return Some(idx);
        }
        idx += 1;
    }

    None
}

pub(crate) fn find_quoted_value_end(value: &str, start: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    if start >= bytes.len() {
        return None;
    }
    let quote = bytes[start];
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == quote && bytes[cursor.saturating_sub(1)] != b'\\' {
            return Some(cursor);
        }
        cursor += 1;
    }
    Some(value.len())
}

pub(crate) fn find_unquoted_value_end(value: &str, start: usize) -> usize {
    let bytes = value.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() {
        let ch = bytes[cursor];
        if ch.is_ascii_whitespace() || ch == b',' || ch == b';' {
            break;
        }
        cursor += 1;
    }
    cursor
}

pub(crate) fn contains_unredacted_assignment(text: &str, key: &str, redaction_token: &str) -> bool {
    let normalized_key = key.trim().to_ascii_lowercase();
    if normalized_key.is_empty() {
        return false;
    }

    for line in text.lines() {
        if line_contains_unredacted_assignment(line, &normalized_key, redaction_token) {
            return true;
        }
    }
    false
}

fn line_contains_unredacted_assignment(line: &str, key: &str, redaction_token: &str) -> bool {
    let bytes = line.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        if !is_key_start(bytes[cursor]) {
            cursor += 1;
            continue;
        }

        let key_start = cursor;
        cursor += 1;
        while cursor < bytes.len() && is_key_char(bytes[cursor]) {
            cursor += 1;
        }
        let key_end = cursor;
        if line[key_start..key_end].to_ascii_lowercase() != key {
            continue;
        }

        let mut value_cursor = cursor;
        while value_cursor < bytes.len() && bytes[value_cursor].is_ascii_whitespace() {
            value_cursor += 1;
        }
        if value_cursor >= bytes.len() || !matches!(bytes[value_cursor], b'=' | b':') {
            continue;
        }

        value_cursor += 1;
        while value_cursor < bytes.len() && bytes[value_cursor].is_ascii_whitespace() {
            value_cursor += 1;
        }
        if value_cursor >= bytes.len() {
            return false;
        }

        let (value_start, value_end) = if matches!(bytes[value_cursor], b'"' | b'\'') {
            let quoted_end = find_quoted_value_end(line, value_cursor).unwrap_or(line.len());
            (value_cursor.saturating_add(1), quoted_end)
        } else {
            (value_cursor, find_unquoted_value_end(line, value_cursor))
        };

        if value_end > value_start {
            let value = &line[value_start..value_end];
            if value != redaction_token {
                return true;
            }
        }

        cursor = value_end.saturating_add(1);
    }

    false
}

pub(crate) fn contains_unredacted_bearer_token(
    text: &str,
    min_token_len: usize,
    redaction_token: &str,
) -> bool {
    let mut search_from = 0;
    while let Some(index) = find_ascii_case_insensitive(text, "bearer ", search_from) {
        let token_start = index + "bearer ".len();
        let token_end = find_unquoted_value_end(text, token_start);
        if token_end.saturating_sub(token_start) >= min_token_len {
            let token = &text[token_start..token_end];
            if token != redaction_token {
                return true;
            }
        }
        search_from = token_end.saturating_add(1);
    }
    false
}

pub(crate) fn contains_unredacted_prefixed_token(
    text: &str,
    prefix: &str,
    min_token_len: usize,
    redaction_token: &str,
) -> bool {
    let mut search_from = 0;
    while let Some(start) = find_ascii_case_insensitive(text, prefix, search_from) {
        let mut end = start + prefix.len();
        while end < text.len() {
            let ch = text.as_bytes()[end];
            if !(ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_' | b'/' | b'+' | b'=')) {
                break;
            }
            end += 1;
        }

        if end.saturating_sub(start) >= min_token_len && &text[start..end] != redaction_token {
            return true;
        }

        search_from = end.saturating_add(1);
    }
    false
}

const fn is_key_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

const fn is_key_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}
