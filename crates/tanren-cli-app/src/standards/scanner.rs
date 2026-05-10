use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::{StandardsCommandError, StandardsError, to_repo_relative_path, validation_failed};

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
    fn parse(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("frontmatter 'name' must not be empty".to_owned());
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
    let directory_path =
        to_repo_relative_path(repository_root, directory).map_err(validation_failed)?;
    if directory_depth > MAX_STANDARDS_DIRECTORY_DEPTH {
        return Err(StandardsCommandError::StandardsParseFailed {
            source: StandardsError::DirectoryDepthLimitExceeded {
                path: directory_path,
                limit: MAX_STANDARDS_DIRECTORY_DEPTH,
            },
        });
    }

    let entries =
        fs::read_dir(directory).map_err(|source| StandardsCommandError::StandardsMissing {
            source: StandardsError::ReadFailure {
                path: directory_path.clone(),
                message: source.to_string(),
            },
        })?;

    for entry_result in entries {
        let entry = entry_result.map_err(|source| StandardsCommandError::StandardsMissing {
            source: StandardsError::ReadFailure {
                path: directory_path.clone(),
                message: source.to_string(),
            },
        })?;
        let path = entry.path();
        let path_relative =
            to_repo_relative_path(repository_root, &path).map_err(validation_failed)?;
        let file_type =
            entry
                .file_type()
                .map_err(|source| StandardsCommandError::StandardsMissing {
                    source: StandardsError::ReadFailure {
                        path: path_relative.clone(),
                        message: source.to_string(),
                    },
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
        return Err(StandardsCommandError::StandardsParseFailed {
            source: StandardsError::MarkdownFileLimitExceeded {
                path: standard_path_relative,
                limit: MAX_STANDARDS_MARKDOWN_FILES,
            },
        });
    }

    let file_bytes = entry
        .metadata()
        .map_err(|source| StandardsCommandError::StandardsParseFailed {
            source: StandardsError::ReadFailure {
                path: standard_path_relative.clone(),
                message: source.to_string(),
            },
        })?
        .len();
    if file_bytes > MAX_STANDARD_FILE_BYTES {
        return Err(StandardsCommandError::StandardsParseFailed {
            source: StandardsError::StandardFileTooLarge {
                path: standard_path_relative,
                limit: MAX_STANDARD_FILE_BYTES,
                actual: file_bytes,
            },
        });
    }

    let total_scanned_bytes = scan_state
        .total_scanned_bytes
        .checked_add(file_bytes)
        .ok_or_else(|| StandardsCommandError::StandardsParseFailed {
            source: StandardsError::StandardsTotalBytesLimitExceeded {
                path: standard_path_relative.clone(),
                limit: MAX_STANDARDS_TOTAL_BYTES,
                actual: u64::MAX,
            },
        })?;
    if total_scanned_bytes > MAX_STANDARDS_TOTAL_BYTES {
        return Err(StandardsCommandError::StandardsParseFailed {
            source: StandardsError::StandardsTotalBytesLimitExceeded {
                path: standard_path_relative,
                limit: MAX_STANDARDS_TOTAL_BYTES,
                actual: total_scanned_bytes,
            },
        });
    }

    let raw =
        fs::read_to_string(path).map_err(|source| StandardsCommandError::StandardsParseFailed {
            source: StandardsError::ReadFailure {
                path: standard_path_relative.clone(),
                message: source.to_string(),
            },
        })?;
    let standard_name = parse_standard_name(&raw, &standard_path_relative)
        .map_err(|source| StandardsCommandError::StandardsParseFailed { source })?;

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
            message: format!("invalid YAML frontmatter: {source}"),
        })?;
    StandardName::parse(&frontmatter.name).map_err(|message| StandardsError::FrontmatterParse {
        path: path.to_owned(),
        message,
    })
}

fn extract_frontmatter<'a>(content: &'a str, path: &str) -> Result<&'a str, StandardsError> {
    let mut lines = content.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return Err(StandardsError::FrontmatterParse {
            path: path.to_owned(),
            message: "missing opening frontmatter delimiter".to_owned(),
        });
    };
    if trim_line_ending(first) != "---" {
        return Err(StandardsError::FrontmatterParse {
            path: path.to_owned(),
            message: "missing opening frontmatter delimiter".to_owned(),
        });
    }

    let body_start = first.len();
    let mut body_end = body_start;

    for line in lines {
        if trim_line_ending(line) == "---" {
            return content.get(body_start..body_end).ok_or_else(|| {
                StandardsError::FrontmatterParse {
                    path: path.to_owned(),
                    message: "invalid frontmatter byte bounds".to_owned(),
                }
            });
        }
        let frontmatter_bytes = body_end
            .checked_add(line.len())
            .and_then(|value| value.checked_sub(body_start))
            .ok_or_else(|| StandardsError::FrontmatterParse {
                path: path.to_owned(),
                message: "frontmatter byte counting overflowed".to_owned(),
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

    Err(StandardsError::FrontmatterParse {
        path: path.to_owned(),
        message: "missing closing frontmatter delimiter".to_owned(),
    })
}

fn trim_line_ending(line: &str) -> &str {
    line.trim_end_matches('\n').trim_end_matches('\r')
}
