use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::Path;

use chrono::{DateTime, Utc};
use tanren_domain::SpecId;
use tanren_domain::methodology::task::RequiredGuard;

use super::super::artifact_projection_fold::{
    FoldedProjectionState, fold_projection_lines, fold_projection_lines_incremental,
};
use super::super::errors::{MethodologyError, MethodologyResult};
use super::{
    CHECKPOINT_ANCHOR_LOOKBACK_BYTES, ProjectionCheckpoint, parse_event_id_from_raw_line,
    parse_phase_event_lines,
};

#[derive(Debug, Clone)]
pub(super) struct FoldedProjectionWithMeta {
    pub(super) folded: FoldedProjectionState,
    pub(super) processed_lines: usize,
    pub(super) processed_bytes: u64,
    pub(super) compacted_at: DateTime<Utc>,
    pub(super) compacted_line_count: usize,
}

pub(super) fn fold_phase_events_file_with_optional_checkpoint(
    spec_id: SpecId,
    phase_events: &Path,
    required_guards: &[RequiredGuard],
    prior_checkpoint: Option<ProjectionCheckpoint>,
    checkpoint_compaction_append_threshold: usize,
) -> MethodologyResult<FoldedProjectionWithMeta> {
    let metadata = std::fs::metadata(phase_events).map_err(|source| MethodologyError::Io {
        path: phase_events.to_path_buf(),
        source,
    })?;
    let file_len = metadata.len();

    if let Some(checkpoint) = prior_checkpoint {
        if checkpoint.spec_id == spec_id
            && checkpoint.processed_bytes <= file_len
            && checkpoint_anchor_matches_at_offset(phase_events, &checkpoint)?
        {
            let tail_lines =
                read_non_empty_lines_from_offset(phase_events, checkpoint.processed_bytes)?;
            let appended_refs = tail_lines.iter().map(String::as_str).collect::<Vec<_>>();
            let appended = parse_phase_event_lines(&appended_refs, checkpoint.processed_lines)?;
            let append_count = appended.len();
            let folded = fold_projection_lines_incremental(
                checkpoint.state,
                spec_id,
                &appended,
                required_guards,
            );
            let processed_lines = checkpoint.processed_lines.saturating_add(append_count);
            let compacted_now = append_count >= checkpoint_compaction_append_threshold;
            let compacted_at = if compacted_now {
                folded.generated_at
            } else {
                checkpoint.compacted_at
            };
            let compacted_line_count = if compacted_now {
                processed_lines
            } else {
                checkpoint.compacted_line_count
            };
            return Ok(FoldedProjectionWithMeta {
                folded,
                processed_lines,
                processed_bytes: file_len,
                compacted_at,
                compacted_line_count,
            });
        }
    }

    let all_lines = read_all_non_empty_lines(phase_events)?;
    let all_refs = all_lines.iter().map(String::as_str).collect::<Vec<_>>();
    let parsed = parse_phase_event_lines(&all_refs, 0)?;
    let folded = fold_projection_lines(spec_id, &parsed, required_guards);
    Ok(FoldedProjectionWithMeta {
        folded: folded.clone(),
        processed_lines: all_lines.len(),
        processed_bytes: file_len,
        compacted_at: folded.generated_at,
        compacted_line_count: all_lines.len(),
    })
}

fn read_all_non_empty_lines(path: &Path) -> MethodologyResult<Vec<String>> {
    let raw = std::fs::read_to_string(path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(raw
        .lines()
        .filter(|line: &&str| !line.trim().is_empty())
        .map(str::to_owned)
        .collect())
}

fn read_non_empty_lines_from_offset(path: &Path, offset: u64) -> MethodologyResult<Vec<String>> {
    let mut file = std::fs::File::open(path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    file.seek(SeekFrom::Start(offset))
        .map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if buf.is_empty() {
        return Ok(Vec::new());
    }
    let text = String::from_utf8(buf).map_err(|err| {
        MethodologyError::Validation(format!("phase-events tail is not UTF-8: {err}"))
    })?;
    Ok(text
        .lines()
        .filter(|line: &&str| !line.trim().is_empty())
        .map(str::to_owned)
        .collect())
}

fn checkpoint_anchor_matches_at_offset(
    path: &Path,
    checkpoint: &ProjectionCheckpoint,
) -> MethodologyResult<bool> {
    if checkpoint.processed_lines == 0 {
        return Ok(checkpoint.processed_bytes == 0);
    }
    if checkpoint.processed_bytes == 0 {
        return Ok(false);
    }
    let Some(expected) = checkpoint.last_event_id else {
        return Ok(false);
    };
    let Some(line) = read_last_non_empty_line_before_offset(path, checkpoint.processed_bytes)?
    else {
        return Ok(false);
    };
    Ok(parse_event_id_from_raw_line(&line).is_some_and(|event_id| event_id == expected))
}

fn read_last_non_empty_line_before_offset(
    path: &Path,
    offset: u64,
) -> MethodologyResult<Option<String>> {
    if offset == 0 {
        return Ok(None);
    }
    let mut file = std::fs::File::open(path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let start = offset.saturating_sub(CHECKPOINT_ANCHOR_LOOKBACK_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let span_len = usize::try_from(offset.saturating_sub(start)).map_err(|err| {
        MethodologyError::Validation(format!("phase-events span overflow: {err}"))
    })?;
    let mut buf = vec![0_u8; span_len];
    file.read_exact(&mut buf)
        .map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if buf.is_empty() {
        return Ok(None);
    }
    let mut end = buf.len();
    while end > 0 && (buf[end - 1] == b'\n' || buf[end - 1] == b'\r') {
        end -= 1;
    }
    if end == 0 {
        return Ok(None);
    }
    let relevant = &buf[..end];
    let line_start = relevant
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |idx| idx + 1);
    if start > 0 && line_start == 0 {
        return Ok(None);
    }
    let line = std::str::from_utf8(&relevant[line_start..]).map_err(|err| {
        MethodologyError::Validation(format!("phase-events anchor line is not UTF-8: {err}"))
    })?;
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(trimmed.to_owned()))
}
