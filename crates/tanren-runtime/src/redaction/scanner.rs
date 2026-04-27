use std::collections::HashSet;

type ByteRange = (usize, usize);
type TokenRanges = (Vec<ByteRange>, Vec<ByteRange>);

#[derive(Debug, Clone)]
struct PrefixTrieNode {
    next: [Option<usize>; 256],
    terminal: bool,
}

impl Default for PrefixTrieNode {
    fn default() -> Self {
        Self {
            next: [None; 256],
            terminal: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledTokenPrefixMatcher {
    nodes: Vec<PrefixTrieNode>,
    prefix_count: usize,
}

impl Default for CompiledTokenPrefixMatcher {
    fn default() -> Self {
        Self {
            nodes: vec![PrefixTrieNode::default()],
            prefix_count: 0,
        }
    }
}

impl CompiledTokenPrefixMatcher {
    pub(crate) fn new(prefixes: &[String]) -> Self {
        let mut matcher = Self::default();
        for prefix in prefixes {
            matcher.insert(prefix);
        }
        matcher
    }

    pub(crate) fn matches(&self, token: &str) -> bool {
        let mut state = 0;
        for byte in token.bytes() {
            let idx = byte.to_ascii_lowercase() as usize;
            let Some(next) = self.nodes[state].next[idx] else {
                return false;
            };
            state = next;
            if self.nodes[state].terminal {
                return true;
            }
        }
        false
    }

    fn insert(&mut self, prefix: &str) {
        let trimmed = prefix.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return;
        }

        let mut state = 0;
        for byte in trimmed.bytes() {
            let idx = byte as usize;
            let next = if let Some(existing) = self.nodes[state].next[idx] {
                existing
            } else {
                let created = self.nodes.len();
                self.nodes.push(PrefixTrieNode::default());
                self.nodes[state].next[idx] = Some(created);
                created
            };
            state = next;
        }

        if !self.nodes[state].terminal {
            self.prefix_count += 1;
            self.nodes[state].terminal = true;
        }
    }
}

#[derive(Debug)]
pub(crate) struct ChannelScanConfig<'a> {
    pub policy_keys: &'a HashSet<String>,
    pub hint_keys: &'a HashSet<String>,
    pub token_prefix_matcher: &'a CompiledTokenPrefixMatcher,
    pub min_token_len: usize,
    pub redaction_token: &'a str,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct ChannelScanArtifacts {
    pub assignment_value_ranges: Vec<ByteRange>,
    pub has_unredacted_sensitive_assignment: bool,
    pub bearer_token_ranges: Vec<ByteRange>,
    pub prefixed_token_ranges: Vec<ByteRange>,
}

impl ChannelScanArtifacts {
    #[must_use]
    pub(crate) fn has_policy_residual_leak(&self) -> bool {
        self.has_unredacted_sensitive_assignment
            || !self.bearer_token_ranges.is_empty()
            || !self.prefixed_token_ranges.is_empty()
    }
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

pub(crate) fn scan_channel(text: &str, config: &ChannelScanConfig<'_>) -> ChannelScanArtifacts {
    let mut artifacts = ChannelScanArtifacts::default();
    scan_assignments(text, |key, value_start, value_end| {
        let is_sensitive = config.policy_keys.contains(key) || config.hint_keys.contains(key);
        let is_unredacted =
            value_end > value_start && &text[value_start..value_end] != config.redaction_token;
        if is_sensitive && is_unredacted {
            artifacts
                .assignment_value_ranges
                .push((value_start, value_end));
            artifacts.has_unredacted_sensitive_assignment = true;
        }
        false
    });
    let (bearer_ranges, prefixed_ranges) = collect_token_style_ranges(
        text,
        config.token_prefix_matcher,
        config.min_token_len,
        config.redaction_token,
    );
    artifacts.bearer_token_ranges = bearer_ranges;
    artifacts.prefixed_token_ranges = prefixed_ranges;
    artifacts
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
) -> Vec<ByteRange> {
    let prefix_matcher = CompiledTokenPrefixMatcher::default();
    let (ranges, _) =
        collect_token_style_ranges(text, &prefix_matcher, min_token_len, redaction_token);
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
) -> Vec<ByteRange> {
    let prefix_matcher = CompiledTokenPrefixMatcher::new(prefixes);
    let (_, ranges) =
        collect_token_style_ranges(text, &prefix_matcher, min_token_len, redaction_token);
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

        let structured_value = matches!(bytes[value_cursor], b'{' | b'[');
        let (value_start, value_end) = if matches!(bytes[value_cursor], b'"' | b'\'') {
            let quoted_end = find_quoted_value_end(text, value_cursor).unwrap_or(text.len());
            (value_cursor.saturating_add(1), quoted_end)
        } else {
            (value_cursor, find_unquoted_value_end(text, value_cursor))
        };

        if visitor(&normalized_key, value_start, value_end) {
            return;
        }

        cursor = if structured_value {
            value_start.saturating_add(1)
        } else {
            value_end.saturating_add(1)
        };
    }
}

fn collect_token_style_ranges(
    text: &str,
    prefix_matcher: &CompiledTokenPrefixMatcher,
    min_token_len: usize,
    redaction_token: &str,
) -> TokenRanges {
    let mut bearer_ranges = Vec::new();
    let mut prefixed_ranges = Vec::new();
    let bytes = text.as_bytes();
    let mut cursor = 0;
    let mut previous_token_is_bearer = false;

    while cursor < bytes.len() {
        while cursor < bytes.len() && !is_secret_token_char(bytes[cursor]) {
            cursor += 1;
        }
        let start = cursor;
        while cursor < bytes.len() && is_secret_token_char(bytes[cursor]) {
            cursor += 1;
        }
        if start == cursor {
            continue;
        }

        let token = &text[start..cursor];
        let token_len = cursor.saturating_sub(start);
        let is_redaction_token = token == redaction_token;

        if previous_token_is_bearer && token_len >= min_token_len && !is_redaction_token {
            bearer_ranges.push((start, cursor));
        }

        if token_len >= min_token_len && !is_redaction_token && prefix_matcher.matches(token) {
            prefixed_ranges.push((start, cursor));
        }

        previous_token_is_bearer = token.eq_ignore_ascii_case("bearer");
    }

    (bearer_ranges, prefixed_ranges)
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

const fn is_secret_token_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'/' | b'+' | b'=' | b'.')
}
