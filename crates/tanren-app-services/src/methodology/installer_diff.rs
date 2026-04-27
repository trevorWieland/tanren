//! Helpers for exact drift diff payloads used by strict dry-run output.

use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

const MAX_TEXT_BYTES: usize = 64 * 1024;
const CONTEXT_LINES: usize = 3;
const MAX_CHANGED_LINES_PER_SIDE: usize = 60;

/// Exact payload for one content mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDiff {
    pub expected_sha256: String,
    pub actual_sha256: String,
    pub unified_diff: String,
}

#[must_use]
pub(crate) fn exact_content_diff(expected: &[u8], actual: &[u8], dest: &Path) -> ContentDiff {
    let expected_sha256 = sha256_hex(expected);
    let actual_sha256 = sha256_hex(actual);
    let unified_diff = build_unified_diff(expected, actual, dest);
    ContentDiff {
        expected_sha256,
        actual_sha256,
        unified_diff,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn build_unified_diff(expected: &[u8], actual: &[u8], dest: &Path) -> String {
    let Some(expected_text) = std::str::from_utf8(expected).ok() else {
        return non_text_summary(expected.len(), actual.len(), dest);
    };
    let Some(actual_text) = std::str::from_utf8(actual).ok() else {
        return non_text_summary(expected.len(), actual.len(), dest);
    };
    if expected.len() > MAX_TEXT_BYTES || actual.len() > MAX_TEXT_BYTES {
        return oversized_text_summary(expected.len(), actual.len(), dest);
    }

    contextual_unified_diff(expected_text, actual_text, dest)
}

fn non_text_summary(expected_len: usize, actual_len: usize, dest: &Path) -> String {
    format!(
        "--- expected:{}\n+++ actual:{}\n@@ non-UTF8/binary @@\ndiff omitted for non-UTF8/binary content (expected={}b, actual={}b)\n",
        dest.display(),
        dest.display(),
        expected_len,
        actual_len,
    )
}

fn oversized_text_summary(expected_len: usize, actual_len: usize, dest: &Path) -> String {
    format!(
        "--- expected:{}\n+++ actual:{}\n@@ text too large @@\ndiff omitted because text exceeds {} bytes (expected={}b, actual={}b)\n",
        dest.display(),
        dest.display(),
        MAX_TEXT_BYTES,
        expected_len,
        actual_len,
    )
}

fn contextual_unified_diff(expected_text: &str, actual_text: &str, dest: &Path) -> String {
    let expected_lines = expected_text.lines().collect::<Vec<_>>();
    let actual_lines = actual_text.lines().collect::<Vec<_>>();

    let mut start = 0usize;
    let common_prefix = expected_lines.len().min(actual_lines.len());
    while start < common_prefix && expected_lines[start] == actual_lines[start] {
        start += 1;
    }

    let mut end_expected = expected_lines.len();
    let mut end_actual = actual_lines.len();
    while end_expected > start
        && end_actual > start
        && expected_lines[end_expected - 1] == actual_lines[end_actual - 1]
    {
        end_expected -= 1;
        end_actual -= 1;
    }

    let ctx_start = start.saturating_sub(CONTEXT_LINES);
    let ctx_end_expected = (end_expected + CONTEXT_LINES).min(expected_lines.len());
    let ctx_end_actual = (end_actual + CONTEXT_LINES).min(actual_lines.len());

    let mut patch = String::new();
    let _ = writeln!(&mut patch, "--- expected:{}", dest.display());
    let _ = writeln!(&mut patch, "+++ actual:{}", dest.display());
    let _ = writeln!(
        &mut patch,
        "@@ -{},{} +{},{} @@",
        ctx_start + 1,
        ctx_end_expected.saturating_sub(ctx_start),
        ctx_start + 1,
        ctx_end_actual.saturating_sub(ctx_start),
    );

    for line in &expected_lines[ctx_start..start] {
        patch.push(' ');
        patch.push_str(line);
        patch.push('\n');
    }
    append_changed_lines(
        &mut patch,
        '-',
        &expected_lines[start..end_expected],
        "removed",
    );
    append_changed_lines(&mut patch, '+', &actual_lines[start..end_actual], "added");
    for line in &expected_lines[end_expected..ctx_end_expected] {
        patch.push(' ');
        patch.push_str(line);
        patch.push('\n');
    }

    patch
}

fn append_changed_lines(out: &mut String, prefix: char, lines: &[&str], label: &str) {
    let truncated = lines.len().saturating_sub(MAX_CHANGED_LINES_PER_SIDE);
    for line in lines.iter().take(MAX_CHANGED_LINES_PER_SIDE) {
        out.push(prefix);
        out.push_str(line);
        out.push('\n');
    }
    if truncated > 0 {
        let _ = writeln!(out, "{prefix}... {truncated} {label} lines truncated ...");
    }
}
