//! Standards markdown tree scanner.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::config::to_repo_relative_path;
use super::error::{StandardsCommandError, StandardsError, StandardsFrontmatterError};

const MAX_STANDARDS_DIRECTORY_DEPTH: usize = 16;
const MAX_STANDARDS_MARKDOWN_FILES: usize = 10_000;
const MAX_STANDARD_FILE_BYTES: u64 = 1_048_576;
const MAX_STANDARDS_TOTAL_BYTES: u64 = 67_108_864;
const MAX_FRONTMATTER_BYTES: usize = 65_536;

#[derive(Debug)]
pub(super) struct StandardsScanSummary {
    pub(super) standards_count: usize,
    pub(super) first_standard_name: Option<String>,
    pub(super) first_standard_path: Option<String>,
}

#[derive(Debug, Clone)]
struct ParsedStandard {
    path: String,
    name: StandardName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StandardName(String);

impl StandardName {
    fn parse(raw: &str) -> Result<Self, StandardsFrontmatterError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(StandardsFrontmatterError::EmptyName);
        }
        Ok(Self(trimmed.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Default)]
struct StandardsScanState {
    standards_count: usize,
    total_scanned_bytes: u64,
    first_standard: Option<ParsedStandard>,
}

impl StandardsScanState {
    fn observe_standard(&mut self, parsed: ParsedStandard) {
        match &self.first_standard {
            Some(first)
                if (parsed.path.as_str(), parsed.name.as_str())
                    >= (first.path.as_str(), first.name.as_str()) => {}
            _ => self.first_standard = Some(parsed),
        }
    }
}

#[derive(Debug, Deserialize)]
struct StandardsFrontmatter {
    name: String,
}

pub(super) fn scan_standards(
    repository_root: &Path,
    standards_root: &Path,
) -> Result<StandardsScanSummary, StandardsCommandError> {
    let mut scan_state = StandardsScanState::default();
    scan_standards_recursive(repository_root, standards_root, 0, &mut scan_state)?;

    let (first_standard_name, first_standard_path) = match scan_state.first_standard {
        Some(first_standard) => (Some(first_standard.name.0), Some(first_standard.path)),
        None => (None, None),
    };

    Ok(StandardsScanSummary {
        standards_count: scan_state.standards_count,
        first_standard_name,
        first_standard_path,
    })
}

fn scan_standards_recursive(
    repository_root: &Path,
    directory: &Path,
    directory_depth: usize,
    scan_state: &mut StandardsScanState,
) -> Result<(), StandardsCommandError> {
    let directory_path = to_repo_relative_path(repository_root, directory)
        .map_err(StandardsCommandError::validation_failed)?;
    if directory_depth > MAX_STANDARDS_DIRECTORY_DEPTH {
        return Err(StandardsCommandError::standards_parse_failed(
            StandardsError::DirectoryDepthLimitExceeded {
                path: directory_path,
                limit: MAX_STANDARDS_DIRECTORY_DEPTH,
            },
        ));
    }

    let entries = fs::read_dir(directory).map_err(|source| {
        StandardsCommandError::standards_missing(StandardsError::ReadFailure {
            path: directory_path.clone(),
            source,
        })
    })?;

    for entry_result in entries {
        let entry = entry_result.map_err(|source| {
            StandardsCommandError::standards_missing(StandardsError::ReadFailure {
                path: directory_path.clone(),
                source,
            })
        })?;
        let path = entry.path();
        let path_relative = to_repo_relative_path(repository_root, &path)
            .map_err(StandardsCommandError::validation_failed)?;
        let file_type = entry.file_type().map_err(|source| {
            StandardsCommandError::standards_missing(StandardsError::ReadFailure {
                path: path_relative.clone(),
                source,
            })
        })?;

        if file_type.is_dir() {
            scan_standards_recursive(repository_root, &path, directory_depth + 1, scan_state)?;
            continue;
        }
        if !file_type.is_file() || path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }

        scan_markdown_file(&entry, &path, path_relative, scan_state)?;
    }

    Ok(())
}

fn scan_markdown_file(
    entry: &fs::DirEntry,
    path: &Path,
    standard_path_relative: String,
    scan_state: &mut StandardsScanState,
) -> Result<(), StandardsCommandError> {
    if scan_state.standards_count >= MAX_STANDARDS_MARKDOWN_FILES {
        return Err(StandardsCommandError::standards_parse_failed(
            StandardsError::MarkdownFileLimitExceeded {
                path: standard_path_relative,
                limit: MAX_STANDARDS_MARKDOWN_FILES,
            },
        ));
    }

    let file_bytes = entry
        .metadata()
        .map_err(|source| {
            StandardsCommandError::standards_parse_failed(StandardsError::ReadFailure {
                path: standard_path_relative.clone(),
                source,
            })
        })?
        .len();
    if file_bytes > MAX_STANDARD_FILE_BYTES {
        return Err(StandardsCommandError::standards_parse_failed(
            StandardsError::StandardFileTooLarge {
                path: standard_path_relative,
                limit: MAX_STANDARD_FILE_BYTES,
                actual: file_bytes,
            },
        ));
    }

    let total_scanned_bytes = scan_state
        .total_scanned_bytes
        .checked_add(file_bytes)
        .ok_or_else(|| {
            StandardsCommandError::standards_parse_failed(
                StandardsError::StandardsTotalBytesLimitExceeded {
                    path: standard_path_relative.clone(),
                    limit: MAX_STANDARDS_TOTAL_BYTES,
                    actual: u64::MAX,
                },
            )
        })?;
    if total_scanned_bytes > MAX_STANDARDS_TOTAL_BYTES {
        return Err(StandardsCommandError::standards_parse_failed(
            StandardsError::StandardsTotalBytesLimitExceeded {
                path: standard_path_relative,
                limit: MAX_STANDARDS_TOTAL_BYTES,
                actual: total_scanned_bytes,
            },
        ));
    }

    let raw = fs::read_to_string(path).map_err(|source| {
        StandardsCommandError::standards_parse_failed(StandardsError::ReadFailure {
            path: standard_path_relative.clone(),
            source,
        })
    })?;
    let standard_name = parse_standard_name(&raw, &standard_path_relative)
        .map_err(StandardsCommandError::standards_parse_failed)?;

    scan_state.standards_count += 1;
    scan_state.total_scanned_bytes = total_scanned_bytes;
    scan_state.observe_standard(ParsedStandard {
        path: standard_path_relative,
        name: standard_name,
    });

    Ok(())
}

fn parse_standard_name(content: &str, path: &str) -> Result<StandardName, StandardsError> {
    let frontmatter = extract_frontmatter(content, path)?;
    let frontmatter: StandardsFrontmatter =
        serde_yaml::from_str(frontmatter).map_err(|source| StandardsError::FrontmatterParse {
            path: path.to_owned(),
            source: StandardsFrontmatterError::from(source),
        })?;
    StandardName::parse(&frontmatter.name).map_err(|source| StandardsError::FrontmatterParse {
        path: path.to_owned(),
        source,
    })
}

fn extract_frontmatter<'a>(content: &'a str, path: &str) -> Result<&'a str, StandardsError> {
    let mut lines = content.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Err(frontmatter_parse_error(
            path,
            StandardsFrontmatterError::MissingOpeningDelimiter,
        ));
    };
    if trim_line_ending(first) != "---" {
        return Err(frontmatter_parse_error(
            path,
            StandardsFrontmatterError::MissingOpeningDelimiter,
        ));
    }

    let body_start = first.len();
    let mut body_end = body_start;

    for line in lines {
        if trim_line_ending(line) == "---" {
            return content.get(body_start..body_end).ok_or_else(|| {
                frontmatter_parse_error(path, StandardsFrontmatterError::InvalidByteBounds)
            });
        }
        let frontmatter_bytes = body_end
            .checked_add(line.len())
            .and_then(|value| value.checked_sub(body_start))
            .ok_or_else(|| {
                frontmatter_parse_error(path, StandardsFrontmatterError::ByteCountingOverflow)
            })?;
        if frontmatter_bytes > MAX_FRONTMATTER_BYTES {
            return Err(StandardsError::FrontmatterTooLarge {
                path: path.to_owned(),
                limit: MAX_FRONTMATTER_BYTES,
                actual: frontmatter_bytes,
            });
        }
        body_end += line.len();
    }

    Err(frontmatter_parse_error(
        path,
        StandardsFrontmatterError::MissingClosingDelimiter,
    ))
}

fn frontmatter_parse_error(path: &str, source: StandardsFrontmatterError) -> StandardsError {
    StandardsError::FrontmatterParse {
        path: path.to_owned(),
        source,
    }
}

fn trim_line_ending(line: &str) -> &str {
    line.trim_end_matches('\n').trim_end_matches('\r')
}
