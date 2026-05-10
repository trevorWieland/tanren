//! Typed install-output contract — parser for `tanren-cli install` stdout.

use thiserror::Error;

/// Parsed install command output — summary counts and affected paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallSummaryOutput {
    created_count: usize,
    updated_count: usize,
    removed_count: usize,
    restored_count: usize,
    preserved_count: usize,
    created_paths: Vec<String>,
    updated_paths: Vec<String>,
    removed_paths: Vec<String>,
    restored_paths: Vec<String>,
    preserved_paths: Vec<String>,
}

impl InstallSummaryOutput {
    /// Number of created paths.
    #[must_use]
    pub const fn created_count(&self) -> usize {
        self.created_count
    }

    /// Number of updated paths.
    #[must_use]
    pub const fn updated_count(&self) -> usize {
        self.updated_count
    }

    /// Number of removed paths.
    #[must_use]
    pub const fn removed_count(&self) -> usize {
        self.removed_count
    }

    /// Number of restored paths.
    #[must_use]
    pub const fn restored_count(&self) -> usize {
        self.restored_count
    }

    /// Number of preserved paths.
    #[must_use]
    pub const fn preserved_count(&self) -> usize {
        self.preserved_count
    }

    /// Whether the install summary reports zero write/materialization operations.
    #[must_use]
    pub fn is_zero_write(&self) -> bool {
        self.created_count == 0
            && self.updated_count == 0
            && self.removed_count == 0
            && self.restored_count == 0
    }

    /// Whether the given path appears in any write-category (created, updated, removed, restored).
    #[must_use]
    pub fn is_path_in_write_categories(&self, path: &str) -> bool {
        self.created_paths.iter().any(|p| p == path)
            || self.updated_paths.iter().any(|p| p == path)
            || self.removed_paths.iter().any(|p| p == path)
            || self.restored_paths.iter().any(|p| p == path)
    }
}

/// Parse error for install summary output contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InstallSummaryOutputParseError {
    /// Summary line is missing or malformed.
    #[error("install output missing valid summary line")]
    MissingSummaryLine,
    /// A required summary field was not found.
    #[error("install summary missing field '{field}'")]
    MissingSummaryField { field: String },
    /// Paths line is missing or malformed.
    #[error("install output missing valid paths line")]
    MissingPathsLine,
}

/// Parse install CLI stdout into a typed [`InstallSummaryOutput`].
///
/// Expected format:
/// ```text
/// status=ok command=install repo=... created=N updated=N removed=N restored=N preserved=N
/// paths created=[path1, path2] updated=[...] removed=[...] restored=[...] preserved=[...]
/// ```
pub fn parse_install_summary_output(
    stdout: &str,
) -> Result<InstallSummaryOutput, InstallSummaryOutputParseError> {
    let mut lines = stdout.lines();

    let summary_line = lines
        .next()
        .ok_or(InstallSummaryOutputParseError::MissingSummaryLine)?;
    let summary_fields = parse_kv_fields(summary_line);

    let created_count = parse_count_field(&summary_fields, "created")?;
    let updated_count = parse_count_field(&summary_fields, "updated")?;
    let removed_count = parse_count_field(&summary_fields, "removed")?;
    let restored_count = parse_count_field(&summary_fields, "restored")?;
    let preserved_count = parse_count_field(&summary_fields, "preserved")?;

    let paths_line = lines
        .next()
        .ok_or(InstallSummaryOutputParseError::MissingPathsLine)?;
    let paths = parse_paths_line(paths_line);

    Ok(InstallSummaryOutput {
        created_count,
        updated_count,
        removed_count,
        restored_count,
        preserved_count,
        created_paths: paths.created,
        updated_paths: paths.updated,
        removed_paths: paths.removed,
        restored_paths: paths.restored,
        preserved_paths: paths.preserved,
    })
}

struct ParsedPaths {
    created: Vec<String>,
    updated: Vec<String>,
    removed: Vec<String>,
    restored: Vec<String>,
    preserved: Vec<String>,
}

fn parse_paths_line(line: &str) -> ParsedPaths {
    let created = extract_bracket_list(line, "created=[").unwrap_or_default();
    let updated = extract_bracket_list(line, "updated=[").unwrap_or_default();
    let removed = extract_bracket_list(line, "removed=[").unwrap_or_default();
    let restored = extract_bracket_list(line, "restored=[").unwrap_or_default();
    let preserved = extract_bracket_list_from_end(line, "preserved=[");
    ParsedPaths {
        created,
        updated,
        removed,
        restored,
        preserved,
    }
}

fn extract_bracket_list(line: &str, start_marker: &str) -> Option<Vec<String>> {
    let start = line.find(start_marker)?;
    let content_start = start + start_marker.len();
    let remaining = &line[content_start..];
    let end = remaining.find(']').unwrap_or(remaining.len());
    Some(parse_comma_list(&remaining[..end]))
}

fn extract_bracket_list_from_end(line: &str, marker: &str) -> Vec<String> {
    let Some(start) = line.rfind(marker) else {
        return Vec::new();
    };
    let content_start = start + marker.len();
    let remaining = &line[content_start..];
    let end = remaining.find(']').unwrap_or(remaining.len());
    parse_comma_list(&remaining[..end])
}

fn parse_comma_list(content: &str) -> Vec<String> {
    if content.is_empty() {
        return Vec::new();
    }
    content
        .split(',')
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .collect()
}

fn parse_kv_fields(line: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for token in line.split_whitespace() {
        if let Some((key, value)) = token.split_once('=') {
            map.insert(key.to_owned(), value.to_owned());
        }
    }
    map
}

fn parse_count_field(
    fields: &std::collections::HashMap<String, String>,
    name: &str,
) -> Result<usize, InstallSummaryOutputParseError> {
    let raw =
        fields
            .get(name)
            .ok_or_else(|| InstallSummaryOutputParseError::MissingSummaryField {
                field: name.to_owned(),
            })?;
    raw.parse::<usize>()
        .map_err(|_| InstallSummaryOutputParseError::MissingSummaryField {
            field: format!("{name}={raw}"),
        })
}
