use super::*;

pub(super) fn phase_events_lock_path(path: &Path) -> std::path::PathBuf {
    let Some(file_name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
        return path.with_extension("lock");
    };
    path.with_file_name(format!("{file_name}.lock"))
}

pub(super) fn jsonl_contains_event_id_locked(
    path: &Path,
    event_id: EventId,
) -> Result<bool, MethodologyError> {
    if !path.exists() {
        return Ok(false);
    }
    let needle = event_id.to_string();
    let needle_fragment = format!("\"event_id\":\"{needle}\"");
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(MethodologyError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let reader = std::io::BufReader::new(file);
    for line in std::io::BufRead::lines(reader) {
        let line = line.map_err(|source| MethodologyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        if !line.contains(&needle_fragment) {
            continue;
        }
        if let Some(found) = parse_event_id_from_line(&line)
            && found == event_id
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn phase_events_event_index_dir(path: &Path) -> std::path::PathBuf {
    let Some(file_name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
        return path.with_extension("event-id-index");
    };
    path.with_file_name(format!(".{file_name}.event-id-index"))
}

pub(super) fn phase_events_fsync_state_path(path: &Path) -> std::path::PathBuf {
    let Some(file_name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
        return path.with_extension("fsync-state.json");
    };
    path.with_file_name(format!(".{file_name}.fsync-state.json"))
}

pub(super) fn event_id_marker_path(path: &Path, event_id: EventId) -> std::path::PathBuf {
    phase_events_event_index_dir(path).join(event_id.to_string())
}

pub(super) fn event_id_marker_exists(path: &Path, event_id: EventId) -> bool {
    path.exists() && event_id_marker_path(path, event_id).exists()
}

pub(super) fn upsert_event_id_marker(
    path: &Path,
    event_id: EventId,
) -> Result<(), MethodologyError> {
    let marker = event_id_marker_path(path, event_id);
    if marker.exists() {
        return Ok(());
    }
    let Some(parent) = marker.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    match std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&marker)
    {
        Ok(mut file) => {
            file.write_all(b"\n")
                .map_err(|source| MethodologyError::Io {
                    path: marker.clone(),
                    source,
                })?;
            Ok(())
        }
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(source) => Err(MethodologyError::Io {
            path: marker,
            source,
        }),
    }
}

pub(super) fn parse_event_id_from_line(line: &str) -> Option<EventId> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let raw = value.get("event_id").and_then(serde_json::Value::as_str)?;
    let parsed = uuid::Uuid::parse_str(raw).ok()?;
    Some(EventId::from_uuid(parsed))
}

pub(super) fn should_sync_after_append(
    path: &Path,
    policy: PhaseEventsAppendPolicy,
) -> Result<bool, MethodologyError> {
    if policy.fsync_every.get() <= 1 {
        reset_fsync_state(path)?;
        return Ok(true);
    }
    let mut state = load_fsync_state(path)?;
    state.pending_since_last_sync = state.pending_since_last_sync.saturating_add(1);
    if state.pending_since_last_sync >= policy.fsync_every.get() {
        state.pending_since_last_sync = 0;
        persist_fsync_state(path, &state)?;
        return Ok(true);
    }
    persist_fsync_state(path, &state)?;
    Ok(false)
}

fn load_fsync_state(path: &Path) -> Result<PhaseEventsFsyncState, MethodologyError> {
    let state_path = phase_events_fsync_state_path(path);
    let raw = match std::fs::read_to_string(&state_path) {
        Ok(raw) => raw,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(PhaseEventsFsyncState {
                schema_version: FSYNC_STATE_SCHEMA_VERSION.to_owned(),
                pending_since_last_sync: 0,
            });
        }
        Err(source) => {
            return Err(MethodologyError::Io {
                path: state_path,
                source,
            });
        }
    };
    let parsed =
        serde_json::from_str::<PhaseEventsFsyncState>(&raw).unwrap_or(PhaseEventsFsyncState {
            schema_version: FSYNC_STATE_SCHEMA_VERSION.to_owned(),
            pending_since_last_sync: 0,
        });
    Ok(PhaseEventsFsyncState {
        schema_version: FSYNC_STATE_SCHEMA_VERSION.to_owned(),
        pending_since_last_sync: parsed.pending_since_last_sync,
    })
}

fn persist_fsync_state(path: &Path, state: &PhaseEventsFsyncState) -> Result<(), MethodologyError> {
    let state_path = phase_events_fsync_state_path(path);
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes =
        serde_json::to_vec(state).map_err(|err| MethodologyError::Validation(err.to_string()))?;
    std::fs::write(&state_path, bytes).map_err(|source| MethodologyError::Io {
        path: state_path,
        source,
    })
}

pub(super) fn reset_fsync_state(path: &Path) -> Result<(), MethodologyError> {
    persist_fsync_state(
        path,
        &PhaseEventsFsyncState {
            schema_version: FSYNC_STATE_SCHEMA_VERSION.to_owned(),
            pending_since_last_sync: 0,
        },
    )
}

pub(super) fn rebuild_event_id_index_locked(
    path: &Path,
    lines: &[String],
) -> Result<(), MethodologyError> {
    let index_dir = phase_events_event_index_dir(path);
    if index_dir.exists() {
        std::fs::remove_dir_all(&index_dir).map_err(|source| MethodologyError::Io {
            path: index_dir.clone(),
            source,
        })?;
    }
    std::fs::create_dir_all(&index_dir).map_err(|source| MethodologyError::Io {
        path: index_dir.clone(),
        source,
    })?;
    for line in lines {
        if let Some(event_id) = parse_event_id_from_line(line) {
            upsert_event_id_marker(path, event_id)?;
        }
    }
    Ok(())
}

pub(super) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), MethodologyError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let mut temp_path = path.to_path_buf();
    let file_name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("phase-events");
    temp_path.set_file_name(format!(".{file_name}.{}.tmp", uuid::Uuid::now_v7()));
    let mut file = std::fs::File::create(&temp_path).map_err(|source| MethodologyError::Io {
        path: temp_path.clone(),
        source,
    })?;
    file.write_all(bytes)
        .map_err(|source| MethodologyError::Io {
            path: temp_path.clone(),
            source,
        })?;
    file.sync_all().map_err(|source| MethodologyError::Io {
        path: temp_path.clone(),
        source,
    })?;
    std::fs::rename(&temp_path, path).map_err(|source| MethodologyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

pub(super) fn compact_jsonl_event_log(
    path: &Path,
) -> Result<PhaseEventsCompactionReport, MethodologyError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| MethodologyError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let lock_path = phase_events_lock_path(path);
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&lock_path)
        .map_err(|source| MethodologyError::Io {
            path: lock_path.clone(),
            source,
        })?;
    lock_file
        .lock_exclusive()
        .map_err(|source| MethodologyError::Io {
            path: lock_path,
            source,
        })?;

    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(source) => {
            return Err(MethodologyError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let mut seen_event_ids = std::collections::HashSet::new();
    let mut kept_lines: Vec<String> = Vec::new();
    let mut total_before = 0_u64;
    let mut duplicates_removed = 0_u64;
    let mut empty_removed = 0_u64;

    for raw_line in raw.lines() {
        total_before = total_before.saturating_add(1);
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            empty_removed = empty_removed.saturating_add(1);
            continue;
        }
        if let Some(event_id) = parse_event_id_from_line(trimmed) {
            if !seen_event_ids.insert(event_id) {
                duplicates_removed = duplicates_removed.saturating_add(1);
                continue;
            }
        }
        kept_lines.push(trimmed.to_owned());
    }

    let normalized = if kept_lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", kept_lines.join("\n"))
    };
    let rewrote_file = raw != normalized;
    if rewrote_file {
        write_atomic(path, normalized.as_bytes())?;
    }

    rebuild_event_id_index_locked(path, &kept_lines)?;
    reset_fsync_state(path)?;
    drop(lock_file);

    Ok(PhaseEventsCompactionReport {
        total_lines_before: total_before,
        total_lines_after: u64::try_from(kept_lines.len()).unwrap_or(u64::MAX),
        duplicates_removed,
        empty_lines_removed: empty_removed,
        rewrote_file,
    })
}
