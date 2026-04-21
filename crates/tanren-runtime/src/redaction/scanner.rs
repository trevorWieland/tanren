use std::collections::HashSet;

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
        if ch.is_ascii_whitespace() || matches!(ch, b',' | b';' | b'}' | b'|' | b'&') {
            break;
        }
        cursor += 1;
    }
    cursor
}

pub(crate) fn collect_assignment_value_ranges(
    text: &str,
    policy_keys: &HashSet<String>,
    hint_keys: &HashSet<String>,
    redaction_token: &str,
) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    scan_assignments(text, |key, value_start, value_end| {
        let is_sensitive = policy_keys.contains(key) || hint_keys.contains(key);
        if is_sensitive
            && value_end > value_start
            && &text[value_start..value_end] != redaction_token
        {
            ranges.push((value_start, value_end));
        }
        false
    });
    ranges
}

pub(crate) fn contains_unredacted_assignment(text: &str, key: &str, redaction_token: &str) -> bool {
    let normalized_key = key.trim().to_ascii_lowercase();
    if normalized_key.is_empty() {
        return false;
    }

    let mut found = false;
    scan_assignments(text, |candidate_key, value_start, value_end| {
        if candidate_key == normalized_key
            && value_end > value_start
            && &text[value_start..value_end] != redaction_token
        {
            found = true;
            return true;
        }
        false
    });
    found
}

pub(crate) fn collect_bearer_token_ranges(
    text: &str,
    min_token_len: usize,
    redaction_token: &str,
) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut search_from = 0;
    while let Some(index) = find_ascii_case_insensitive(text, "bearer ", search_from) {
        let token_start = index + "bearer ".len();
        let token_end = find_unquoted_value_end(text, token_start);
        if token_end.saturating_sub(token_start) >= min_token_len
            && &text[token_start..token_end] != redaction_token
        {
            ranges.push((token_start, token_end));
        }
        search_from = token_end.saturating_add(1);
    }
    ranges
}

pub(crate) fn contains_unredacted_bearer_token(
    text: &str,
    min_token_len: usize,
    redaction_token: &str,
) -> bool {
    !collect_bearer_token_ranges(text, min_token_len, redaction_token).is_empty()
}

pub(crate) fn collect_prefixed_token_ranges(
    text: &str,
    prefixes: &[String],
    min_token_len: usize,
    redaction_token: &str,
) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for prefix in prefixes {
        let mut search_from = 0;
        while let Some(start) = find_ascii_case_insensitive(text, prefix, search_from) {
            let mut end = start + prefix.len();
            while end < text.len() {
                let ch = text.as_bytes()[end];
                if !(ch.is_ascii_alphanumeric()
                    || matches!(ch, b'-' | b'_' | b'/' | b'+' | b'=' | b'.'))
                {
                    break;
                }
                end += 1;
            }

            if end.saturating_sub(start) >= min_token_len && &text[start..end] != redaction_token {
                ranges.push((start, end));
            }

            search_from = end.saturating_add(1);
        }
    }
    ranges
}

pub(crate) fn contains_unredacted_prefixed_token(
    text: &str,
    prefix: &str,
    min_token_len: usize,
    redaction_token: &str,
) -> bool {
    !collect_prefixed_token_ranges(text, &[prefix.to_owned()], min_token_len, redaction_token)
        .is_empty()
}

fn scan_assignments(text: &str, mut visitor: impl FnMut(&str, usize, usize) -> bool) {
    let bytes = text.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        let Some((normalized_key, key_end)) = parse_assignment_key(text, cursor) else {
            cursor += 1;
            continue;
        };

        let mut value_cursor = key_end;
        while value_cursor < bytes.len() && bytes[value_cursor].is_ascii_whitespace() {
            value_cursor += 1;
        }
        if value_cursor >= bytes.len() || !matches!(bytes[value_cursor], b'=' | b':') {
            cursor = key_end;
            continue;
        }

        value_cursor += 1;
        while value_cursor < bytes.len() && bytes[value_cursor].is_ascii_whitespace() {
            value_cursor += 1;
        }
        if value_cursor >= bytes.len() {
            return;
        }

        let (value_start, value_end) = if matches!(bytes[value_cursor], b'"' | b'\'') {
            let quoted_end = find_quoted_value_end(text, value_cursor).unwrap_or(text.len());
            (value_cursor.saturating_add(1), quoted_end)
        } else {
            (value_cursor, find_unquoted_value_end(text, value_cursor))
        };

        if visitor(&normalized_key, value_start, value_end) {
            return;
        }

        cursor = key_end;
    }
}

fn parse_assignment_key(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if start >= bytes.len() {
        return None;
    }

    if matches!(bytes[start], b'"' | b'\'') {
        let key_end_quote = find_quoted_value_end(text, start)?;
        let key_start = start.saturating_add(1);
        let key = text[key_start..key_end_quote].trim().to_ascii_lowercase();
        if key.is_empty() {
            return None;
        }
        return Some((key, key_end_quote.saturating_add(1)));
    }

    if !is_key_start(bytes[start]) {
        return None;
    }

    let mut cursor = start + 1;
    while cursor < bytes.len() && is_key_char(bytes[cursor]) {
        cursor += 1;
    }
    let key = text[start..cursor].to_ascii_lowercase();
    Some((key, cursor))
}

const fn is_key_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

const fn is_key_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}
