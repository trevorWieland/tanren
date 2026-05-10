//! Typed domain identifiers and closed action vocabulary for the
//! upgrade-fixture test-hook route.

use std::fmt;
use std::str::FromStr;

use super::super::limits;
use axum::http::StatusCode;
use serde::Deserialize;
use serde::Serialize;

/// Scenario-scoped fixture identifier — each BDD scenario keys its
/// repository fixture state under a [`FixtureId`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct FixtureId(String);

impl FixtureId {
    pub(crate) const DEFAULT: &'static str = "default";
    pub(crate) fn parse(value: &str) -> Result<Self, (StatusCode, String)> {
        limits::validate_fixture_id(value.trim())?;
        Ok(Self(value.trim().to_owned()))
    }
}

impl fmt::Display for FixtureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Validated snapshot label — non-empty, length-bounded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct SnapshotLabel(String);

impl SnapshotLabel {
    pub(crate) fn parse(value: &str) -> Result<Self, (StatusCode, String)> {
        limits::validate_snapshot_label(value)?;
        Ok(Self(value.trim().to_owned()))
    }
}

impl fmt::Display for SnapshotLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Closed action vocabulary — the compiler enforces exhaustiveness
/// at every match site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FixtureAction {
    Reset,
    WriteFile,
    RecordBaseline,
    SeedInstall,
    MarkLegacyMigrationConcern,
    CaptureSnapshot,
    RunUpgradePreview,
    RunUpgradeApply,
    AssertNoWrites,
    AssertMatchesSnapshot,
    AssertPreservesBaseline,
    AssertReplacedFromBaseline,
    AssertFileMissing,
    LastRun,
}

impl FromStr for FixtureAction {
    type Err = (StatusCode, String);
    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "reset" => Ok(Self::Reset),
            "write-file" => Ok(Self::WriteFile),
            "record-baseline" => Ok(Self::RecordBaseline),
            "seed-install" => Ok(Self::SeedInstall),
            "mark-legacy-migration-concern" => Ok(Self::MarkLegacyMigrationConcern),
            "capture-snapshot" => Ok(Self::CaptureSnapshot),
            "run-upgrade-preview" => Ok(Self::RunUpgradePreview),
            "run-upgrade-apply" => Ok(Self::RunUpgradeApply),
            "assert-no-writes" => Ok(Self::AssertNoWrites),
            "assert-matches-snapshot" => Ok(Self::AssertMatchesSnapshot),
            "assert-preserves-baseline" => Ok(Self::AssertPreservesBaseline),
            "assert-replaced-from-baseline" => Ok(Self::AssertReplacedFromBaseline),
            "assert-file-missing" => Ok(Self::AssertFileMissing),
            "last-run" => Ok(Self::LastRun),
            unknown => Err((
                StatusCode::NOT_FOUND,
                format!("unknown upgrade-fixture action '{unknown}'"),
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CommandResult {
    pub(super) stdout: String,
    pub(super) status: i32,
    pub(super) success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) apply_outcome: Option<tanren_delivery::install::UpgradeApplyOutcome>,
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct FileWriteBody {
    pub(super) path: String,
    pub(super) content: String,
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct FilePathBody {
    pub(super) path: String,
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct InstallSeedBody {
    pub(super) snapshot_label: String,
    pub(super) profile: String,
    pub(super) integrations: String,
}

#[derive(Debug, serde::Deserialize)]
pub(super) struct SnapshotBody {
    pub(super) label: String,
}
