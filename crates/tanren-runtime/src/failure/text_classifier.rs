use super::HarnessFailureClass;
use super::taxonomy::FAILURE_TAXONOMY;

const MAX_ANALYZED_CHANNEL_BYTES: usize = 16 * 1024;

pub(super) fn classify_text_fallback(
    stdout_tail: Option<&str>,
    stderr_tail: Option<&str>,
) -> HarnessFailureClass {
    let mut tokens = Vec::new();
    if let Some(stdout) = stdout_tail {
        collect_tokens(tail_slice(stdout), &mut tokens);
    }
    if let Some(stderr) = stderr_tail {
        collect_tokens(tail_slice(stderr), &mut tokens);
    }

    for rule in FAILURE_TAXONOMY {
        if matches_rule(rule, &tokens) {
            return rule.class;
        }
    }

    HarnessFailureClass::Unknown
}

fn matches_rule(rule: &super::taxonomy::FailureTaxonomyRule, tokens: &[String]) -> bool {
    if rule
        .text_tokens
        .iter()
        .any(|candidate| tokens.iter().any(|token| token == candidate))
    {
        return true;
    }

    rule.text_phrases
        .iter()
        .any(|phrase| contains_phrase(tokens, phrase))
}

fn contains_phrase(tokens: &[String], phrase: &[&str]) -> bool {
    if phrase.is_empty() || tokens.len() < phrase.len() {
        return false;
    }

    tokens.windows(phrase.len()).any(|window| {
        window
            .iter()
            .zip(phrase.iter())
            .all(|(actual, expected)| actual == expected)
    })
}

fn tail_slice(value: &str) -> &str {
    if value.len() <= MAX_ANALYZED_CHANNEL_BYTES {
        return value;
    }

    let mut start = value.len() - MAX_ANALYZED_CHANNEL_BYTES;
    while start < value.len() && !value.is_char_boundary(start) {
        start += 1;
    }
    &value[start..]
}

fn collect_tokens(text: &str, sink: &mut Vec<String>) {
    let bytes = text.as_bytes();
    let mut cursor = 0;

    while cursor < bytes.len() {
        while cursor < bytes.len() && !is_token_char(bytes[cursor]) {
            cursor += 1;
        }
        let start = cursor;
        while cursor < bytes.len() && is_token_char(bytes[cursor]) {
            cursor += 1;
        }
        if start == cursor {
            continue;
        }

        sink.push(text[start..cursor].to_ascii_lowercase());
    }
}

const fn is_token_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}
