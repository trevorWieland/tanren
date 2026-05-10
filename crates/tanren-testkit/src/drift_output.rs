//! Typed drift output contract — parser for `tanren-cli drift` stdout.

use thiserror::Error;

/// Drift output command status reported by `tanren-cli drift`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftCommandStatus {
    /// No install-managed drift detected.
    Ok,
    /// One or more install-managed paths drifted from the projection.
    Drift,
}

impl DriftCommandStatus {
    #[must_use]
    pub const fn is_drift(self) -> bool {
        matches!(self, Self::Drift)
    }
}

/// Typed drift status for a single install-managed path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftPathStatus {
    /// Repository file matches the current install projection.
    Clean,
    /// Tanren-owned generated file exists but content differs from projection.
    ChangedGenerated,
    /// Tanren-owned generated file is missing from the repository.
    MissingGenerated,
    /// Preserved standards profile file is missing from the repository.
    MissingPreserved,
    /// Preserved standards profile file was edited and accepted as user-owned.
    AcceptedPreserved,
}

/// One bounded drift detail record emitted by `tanren-cli drift`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftDetailRecord {
    path: String,
    status: DriftPathStatus,
}

impl DriftDetailRecord {
    /// Repo-relative path for this detail record.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Typed drift status for this record.
    #[must_use]
    pub const fn status(&self) -> DriftPathStatus {
        self.status
    }
}

/// Parsed drift output — summary line plus per-path detail records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftOutput {
    status: DriftCommandStatus,
    clean_count: usize,
    changed_generated_count: usize,
    missing_generated_count: usize,
    missing_preserved_count: usize,
    accepted_preserved_count: usize,
    drift_count: usize,
    details: Vec<DriftDetailRecord>,
}

impl DriftOutput {
    /// Overall drift command status.
    #[must_use]
    pub const fn status(&self) -> DriftCommandStatus {
        self.status
    }

    /// Number of clean paths.
    #[must_use]
    pub const fn clean_count(&self) -> usize {
        self.clean_count
    }

    /// Number of changed generated assets.
    #[must_use]
    pub const fn changed_generated_count(&self) -> usize {
        self.changed_generated_count
    }

    /// Number of missing generated assets.
    #[must_use]
    pub const fn missing_generated_count(&self) -> usize {
        self.missing_generated_count
    }

    /// Number of missing preserved standards.
    #[must_use]
    pub const fn missing_preserved_count(&self) -> usize {
        self.missing_preserved_count
    }

    /// Number of accepted preserved edits.
    #[must_use]
    pub const fn accepted_preserved_count(&self) -> usize {
        self.accepted_preserved_count
    }

    /// Total number of paths classified as drift.
    #[must_use]
    pub const fn drift_count(&self) -> usize {
        self.drift_count
    }

    /// Per-path detail records for non-clean entries.
    #[must_use]
    pub fn details(&self) -> &[DriftDetailRecord] {
        &self.details
    }

    /// Whether any drift was detected.
    #[must_use]
    pub fn has_drift(&self) -> bool {
        self.status.is_drift()
    }
}

/// Parse error for drift output contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DriftOutputParseError {
    /// Summary line is missing or malformed.
    #[error("drift output missing valid summary line")]
    MissingSummaryLine,
    /// A required summary field was not found.
    #[error("drift summary missing field '{field}'")]
    MissingSummaryField { field: String },
    /// A detail record line is malformed.
    #[error("drift detail record malformed at line {line_number}: {message}")]
    MalformedDetailRecord { line_number: usize, message: String },
    /// An unknown drift path status was encountered.
    #[error("unknown drift path status '{status}'")]
    UnknownPathStatus { status: String },
}

/// Parse drift CLI stdout into a typed [`DriftOutput`] in a single pass.
///
/// Expected format:
/// ```text
/// summary status=<ok|drift> command=drift ... clean=<n> changed_generated=<n> ... drift=<n>
/// detail status=<status> path=<path>
/// detail status=<status> path=<path>
/// ...
/// ```
pub fn parse_drift_output(stdout: &str) -> Result<DriftOutput, DriftOutputParseError> {
    let mut lines = stdout.lines().peekable();

    let summary_line = lines
        .next()
        .ok_or(DriftOutputParseError::MissingSummaryLine)?;
    let summary = parse_summary_line(summary_line)?;

    let mut details = Vec::new();
    for (idx, line) in lines.enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with("detail ") {
            continue;
        }
        let record = parse_detail_record(trimmed, idx + 2).map_err(|(msg, line_num)| {
            DriftOutputParseError::MalformedDetailRecord {
                line_number: line_num,
                message: msg,
            }
        })?;
        details.push(record);
    }

    Ok(DriftOutput {
        status: summary.status,
        clean_count: summary.clean_count,
        changed_generated_count: summary.changed_generated_count,
        missing_generated_count: summary.missing_generated_count,
        missing_preserved_count: summary.missing_preserved_count,
        accepted_preserved_count: summary.accepted_preserved_count,
        drift_count: summary.drift_count,
        details,
    })
}

struct SummaryFields {
    status: DriftCommandStatus,
    clean_count: usize,
    changed_generated_count: usize,
    missing_generated_count: usize,
    missing_preserved_count: usize,
    accepted_preserved_count: usize,
    drift_count: usize,
}

fn parse_summary_line(line: &str) -> Result<SummaryFields, DriftOutputParseError> {
    let fields = parse_kv_fields(line);

    let status_str =
        fields
            .get("status")
            .ok_or_else(|| DriftOutputParseError::MissingSummaryField {
                field: "status".to_owned(),
            })?;
    let status = match status_str.as_str() {
        "ok" => DriftCommandStatus::Ok,
        "drift" => DriftCommandStatus::Drift,
        other => {
            return Err(DriftOutputParseError::MissingSummaryField {
                field: format!("status={other}"),
            });
        }
    };

    let clean_count = parse_count_field(&fields, "clean")?;
    let changed_generated_count = parse_count_field(&fields, "changed_generated")?;
    let missing_generated_count = parse_count_field(&fields, "missing_generated")?;
    let missing_preserved_count = parse_count_field(&fields, "missing_preserved")?;
    let accepted_preserved_count = parse_count_field(&fields, "accepted_preserved")?;
    let drift_count = parse_count_field(&fields, "drift")?;

    Ok(SummaryFields {
        status,
        clean_count,
        changed_generated_count,
        missing_generated_count,
        missing_preserved_count,
        accepted_preserved_count,
        drift_count,
    })
}

fn parse_count_field(
    fields: &std::collections::HashMap<String, String>,
    name: &str,
) -> Result<usize, DriftOutputParseError> {
    let raw = fields
        .get(name)
        .ok_or_else(|| DriftOutputParseError::MissingSummaryField {
            field: name.to_owned(),
        })?;
    raw.parse::<usize>()
        .map_err(|_| DriftOutputParseError::MissingSummaryField {
            field: format!("{name}={raw}"),
        })
}

fn parse_detail_record(
    line: &str,
    line_number: usize,
) -> Result<DriftDetailRecord, (String, usize)> {
    let fields = parse_kv_fields(line);
    let status_str = fields
        .get("status")
        .ok_or_else(|| ("missing 'status' field".to_owned(), line_number))?;
    let path = fields
        .get("path")
        .ok_or_else(|| ("missing 'path' field".to_owned(), line_number))?;

    let status = match status_str.as_str() {
        "clean" => DriftPathStatus::Clean,
        "changed_generated" => DriftPathStatus::ChangedGenerated,
        "missing_generated" => DriftPathStatus::MissingGenerated,
        "missing_preserved" => DriftPathStatus::MissingPreserved,
        "accepted_preserved" => DriftPathStatus::AcceptedPreserved,
        other => {
            return Err(DriftOutputParseError::UnknownPathStatus {
                status: other.to_owned(),
            }
            .into());
        }
    };

    Ok(DriftDetailRecord {
        path: path.clone(),
        status,
    })
}

impl From<DriftOutputParseError> for (String, usize) {
    fn from(_: DriftOutputParseError) -> Self {
        ("parse error".to_owned(), 0)
    }
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
