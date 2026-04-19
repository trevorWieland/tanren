//! Helpers for exact drift diff payloads used by strict dry-run output.

use std::fmt::Write as _;
use std::path::Path;

use sha2::{Digest, Sha256};

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
    let expected_text = String::from_utf8_lossy(expected);
    let actual_text = String::from_utf8_lossy(actual);
    let expected_lines = expected_text.lines().collect::<Vec<_>>();
    let actual_lines = actual_text.lines().collect::<Vec<_>>();
    let mut patch = String::new();
    let _ = writeln!(&mut patch, "--- expected:{}", dest.display());
    let _ = writeln!(&mut patch, "+++ actual:{}", dest.display());
    let _ = writeln!(
        &mut patch,
        "@@ -1,{} +1,{} @@",
        expected_lines.len(),
        actual_lines.len()
    );
    for line in expected_lines {
        patch.push('-');
        patch.push_str(line);
        patch.push('\n');
    }
    for line in actual_lines {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }
    ContentDiff {
        expected_sha256,
        actual_sha256,
        unified_diff: patch,
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
