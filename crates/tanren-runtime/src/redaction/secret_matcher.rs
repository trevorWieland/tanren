use std::collections::{HashMap, HashSet};

use crate::redaction::RedactionHints;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecretCandidate {
    bytes: Vec<u8>,
    contextual_short: bool,
}

#[derive(Default)]
pub(super) struct CompiledSecretMatcher {
    by_first_byte: HashMap<u8, Vec<SecretCandidate>>,
}

impl CompiledSecretMatcher {
    pub(super) fn from_hints(hints: &RedactionHints, min_secret_fragment_len: usize) -> Self {
        let mut literals = HashSet::new();
        let mut contextual_short_literals = HashSet::new();
        for secret in &hints.secret_values {
            let value = secret.expose().trim();
            if value.is_empty() {
                continue;
            }
            literals.insert(value.to_owned());
            add_encoded_variants(value, min_secret_fragment_len, &mut literals);
            if value.contains('\n') {
                for fragment in value.lines() {
                    let trimmed = fragment.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.len() >= min_secret_fragment_len {
                        literals.insert(trimmed.to_owned());
                        add_encoded_variants(trimmed, min_secret_fragment_len, &mut literals);
                    } else if should_track_contextual_short_fragment(
                        trimmed,
                        min_secret_fragment_len,
                    ) {
                        contextual_short_literals.insert(trimmed.to_owned());
                    }
                }
            }
        }

        let mut by_first_byte: HashMap<u8, Vec<SecretCandidate>> = HashMap::new();
        for literal in literals {
            let bytes = literal.into_bytes();
            if let Some(first) = bytes.first().copied() {
                by_first_byte
                    .entry(first)
                    .or_default()
                    .push(SecretCandidate {
                        bytes,
                        contextual_short: false,
                    });
            }
        }
        for literal in contextual_short_literals {
            let bytes = literal.into_bytes();
            if let Some(first) = bytes.first().copied() {
                by_first_byte
                    .entry(first)
                    .or_default()
                    .push(SecretCandidate {
                        bytes,
                        contextual_short: true,
                    });
            }
        }
        for candidates in by_first_byte.values_mut() {
            candidates.sort_by_key(|value| std::cmp::Reverse(value.bytes.len()));
        }
        Self { by_first_byte }
    }

    pub(super) fn collect_ranges(&self, text: &str) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        let bytes = text.as_bytes();
        let mut cursor = 0;

        while cursor < bytes.len() {
            let Some(candidates) = self.by_first_byte.get(&bytes[cursor]) else {
                cursor += 1;
                continue;
            };

            let mut matched = false;
            for candidate in candidates {
                let end = cursor.saturating_add(candidate.bytes.len());
                if end > bytes.len()
                    || &bytes[cursor..end] != candidate.bytes.as_slice()
                    || !text.is_char_boundary(cursor)
                    || !text.is_char_boundary(end)
                {
                    continue;
                }
                if candidate.contextual_short
                    && !is_contextual_short_fragment_match(bytes, cursor, end)
                {
                    continue;
                }
                ranges.push((cursor, end));
                cursor = end;
                matched = true;
                break;
            }
            if !matched {
                cursor += 1;
            }
        }
        ranges
    }
}

fn should_track_contextual_short_fragment(fragment: &str, min_secret_fragment_len: usize) -> bool {
    const MIN_CONTEXTUAL_FRAGMENT_LEN: usize = 3;
    fragment.len() >= MIN_CONTEXTUAL_FRAGMENT_LEN
        && fragment.len() < min_secret_fragment_len
        && fragment
            .bytes()
            .any(|byte| byte.is_ascii_digit() || !byte.is_ascii_alphanumeric())
}

fn is_contextual_short_fragment_match(text: &[u8], start: usize, end: usize) -> bool {
    let prev = start.checked_sub(1).and_then(|idx| text.get(idx)).copied();
    let next = text.get(end).copied();
    let has_boundaries = is_secret_token_boundary(prev) && is_secret_token_boundary(next);
    let has_context_delimiter = prev.is_some_and(is_secret_context_delimiter)
        || next.is_some_and(is_secret_context_delimiter);
    has_boundaries && has_context_delimiter
}

const fn is_secret_token_boundary(ch: Option<u8>) -> bool {
    match ch {
        None => true,
        Some(byte) => !is_secret_token_char(byte),
    }
}

const fn is_secret_context_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b'=' | b':' | b'"' | b'\'' | b'%' | b'?' | b'&' | b'/' | b'\\' | b'+' | b'.'
    )
}

const fn is_secret_token_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'/' | b'+' | b'=' | b'.')
}

fn add_encoded_variants(secret: &str, min_secret_fragment_len: usize, sink: &mut HashSet<String>) {
    const MAX_ENCODE_SOURCE_LEN: usize = 64;
    if secret.len() < min_secret_fragment_len || secret.len() > MAX_ENCODE_SOURCE_LEN {
        return;
    }

    let percent_encoded = percent_encode(secret.as_bytes());
    if percent_encoded.len() >= min_secret_fragment_len {
        sink.insert(percent_encoded);
    }

    let standard = base64_encode(
        secret.as_bytes(),
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
        true,
    );
    if standard.len() >= min_secret_fragment_len {
        sink.insert(standard);
    }

    let urlsafe = base64_encode(
        secret.as_bytes(),
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
        false,
    );
    if urlsafe.len() >= min_secret_fragment_len {
        sink.insert(urlsafe);
    }
}

pub(super) fn percent_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len().saturating_mul(3));
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[(byte >> 4) as usize]));
            out.push(char::from(HEX[(byte & 0x0F) as usize]));
        }
    }
    out
}

pub(super) fn base64_encode(input: &[u8], table: &[u8; 64], include_padding: bool) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3).saturating_mul(4));
    let mut idx = 0;
    while idx + 3 <= input.len() {
        let chunk = u32::from(input[idx]) << 16
            | u32::from(input[idx + 1]) << 8
            | u32::from(input[idx + 2]);
        out.push(char::from(table[((chunk >> 18) & 0x3F) as usize]));
        out.push(char::from(table[((chunk >> 12) & 0x3F) as usize]));
        out.push(char::from(table[((chunk >> 6) & 0x3F) as usize]));
        out.push(char::from(table[(chunk & 0x3F) as usize]));
        idx += 3;
    }

    let remainder = input.len().saturating_sub(idx);
    if remainder == 1 {
        let chunk = u32::from(input[idx]) << 16;
        out.push(char::from(table[((chunk >> 18) & 0x3F) as usize]));
        out.push(char::from(table[((chunk >> 12) & 0x3F) as usize]));
        if include_padding {
            out.push('=');
            out.push('=');
        }
    } else if remainder == 2 {
        let chunk = u32::from(input[idx]) << 16 | u32::from(input[idx + 1]) << 8;
        out.push(char::from(table[((chunk >> 18) & 0x3F) as usize]));
        out.push(char::from(table[((chunk >> 12) & 0x3F) as usize]));
        out.push(char::from(table[((chunk >> 6) & 0x3F) as usize]));
        if include_padding {
            out.push('=');
        }
    } else if remainder > 2 {
        // `idx` advances in 3-byte chunks, so this branch is unreachable.
        // Keep explicit handling to satisfy exhaustive reasoning without panic paths.
        let chunk = u32::from(input[idx]) << 16
            | u32::from(input[idx + 1]) << 8
            | u32::from(input[idx + 2]);
        out.push(char::from(table[((chunk >> 18) & 0x3F) as usize]));
        out.push(char::from(table[((chunk >> 12) & 0x3F) as usize]));
        out.push(char::from(table[((chunk >> 6) & 0x3F) as usize]));
        out.push(char::from(table[(chunk & 0x3F) as usize]));
    }

    out
}
