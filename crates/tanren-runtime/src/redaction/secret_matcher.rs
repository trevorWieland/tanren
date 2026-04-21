use std::collections::{HashMap, HashSet, VecDeque};

use crate::redaction::RedactionHints;

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecretCandidate {
    bytes: Vec<u8>,
    contextual_short: bool,
}

#[derive(Debug, Clone, Default)]
struct AutomatonNode {
    next: HashMap<u8, usize>,
    fail: usize,
    outputs: Vec<usize>,
}

#[derive(Debug, Default)]
pub(super) struct CompiledSecretMatcher {
    nodes: Vec<AutomatonNode>,
    candidates: Vec<SecretCandidate>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SecretMatcherStats {
    pub states: usize,
    pub transitions: usize,
    pub patterns: usize,
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

        let mut candidates = Vec::with_capacity(literals.len() + contextual_short_literals.len());
        for literal in literals {
            candidates.push(SecretCandidate {
                bytes: literal.into_bytes(),
                contextual_short: false,
            });
        }
        for literal in contextual_short_literals {
            candidates.push(SecretCandidate {
                bytes: literal.into_bytes(),
                contextual_short: true,
            });
        }

        Self::from_candidates(candidates)
    }

    fn from_candidates(mut candidates: Vec<SecretCandidate>) -> Self {
        if candidates.is_empty() {
            return Self::default();
        }

        // Sort by length descending so ranges stay deterministic for overlapping outputs.
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.bytes.len()));

        let mut nodes = vec![AutomatonNode::default()];
        for (idx, candidate) in candidates.iter().enumerate() {
            let mut state = 0;
            for &byte in &candidate.bytes {
                let next = if let Some(existing) = nodes[state].next.get(&byte).copied() {
                    existing
                } else {
                    let created = nodes.len();
                    nodes.push(AutomatonNode::default());
                    nodes[state].next.insert(byte, created);
                    created
                };
                state = next;
            }
            nodes[state].outputs.push(idx);
        }

        let mut queue = VecDeque::new();
        let root_children = nodes[0].next.values().copied().collect::<Vec<_>>();
        for child in root_children {
            nodes[child].fail = 0;
            queue.push_back(child);
        }

        while let Some(state) = queue.pop_front() {
            let transitions = nodes[state]
                .next
                .iter()
                .map(|(&byte, &next)| (byte, next))
                .collect::<Vec<_>>();
            for (byte, next_state) in transitions {
                queue.push_back(next_state);

                let mut fail = nodes[state].fail;
                while fail != 0 && !nodes[fail].next.contains_key(&byte) {
                    fail = nodes[fail].fail;
                }

                if let Some(&fallback) = nodes[fail].next.get(&byte) {
                    nodes[next_state].fail = fallback;
                }

                let fallback_outputs = nodes[nodes[next_state].fail].outputs.clone();
                nodes[next_state].outputs.extend(fallback_outputs);
            }
        }

        for node in &mut nodes {
            node.outputs
                .sort_by_key(|idx| std::cmp::Reverse(candidates[*idx].bytes.len()));
            node.outputs.dedup();
        }

        Self { nodes, candidates }
    }

    pub(super) fn collect_ranges(&self, text: &str) -> Vec<(usize, usize)> {
        if self.nodes.is_empty() || self.candidates.is_empty() {
            return Vec::new();
        }

        let bytes = text.as_bytes();
        let mut state = 0;
        let mut ranges = Vec::new();

        for (idx, &byte) in bytes.iter().enumerate() {
            while state != 0 && !self.nodes[state].next.contains_key(&byte) {
                state = self.nodes[state].fail;
            }

            if let Some(&next) = self.nodes[state].next.get(&byte) {
                state = next;
            }

            if self.nodes[state].outputs.is_empty() {
                continue;
            }

            for &candidate_idx in &self.nodes[state].outputs {
                let candidate = &self.candidates[candidate_idx];
                let end = idx.saturating_add(1);
                let Some(start) = end.checked_sub(candidate.bytes.len()) else {
                    continue;
                };
                if !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                    continue;
                }
                if candidate.contextual_short
                    && !is_contextual_short_fragment_match(bytes, start, end)
                {
                    continue;
                }
                ranges.push((start, end));
            }
        }

        ranges
    }

    #[cfg(test)]
    pub(super) fn stats(&self) -> SecretMatcherStats {
        let transition_count = self.nodes.iter().map(|node| node.next.len()).sum();
        SecretMatcherStats {
            states: self.nodes.len(),
            transitions: transition_count,
            patterns: self.candidates.len(),
        }
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
    if secret.len() < min_secret_fragment_len {
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
