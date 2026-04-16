use std::io::{BufRead as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{RecvTimeoutError, SyncSender, sync_channel};
use std::time::Duration;

use crate::ObservabilityError;

pub const INTERNAL_ERROR_SINK_PATH_ENV_VAR: &str = "TANREN_INTERNAL_ERROR_SINK_PATH";

const DEFAULT_QUEUE_CAPACITY: usize = 512;
const DEFAULT_ACK_TIMEOUT: Duration = Duration::from_millis(75);
const DEFAULT_WRITE_DELAY: Duration = Duration::ZERO;
const SINK_FILENAME: &str = "internal-errors.jsonl";
const SPOOL_SUFFIX: &str = ".spool";
const DRAINING_SUFFIX: &str = ".draining";
const RETRY_ATTEMPTS: usize = 3;
const RETRY_BACKOFF: Duration = Duration::from_millis(20);

#[derive(Debug, Clone)]
pub(crate) struct SinkPaths {
    pub(crate) primary: PathBuf,
    pub(crate) spool: PathBuf,
}

#[derive(Debug)]
struct SinkWriteRequest {
    paths: SinkPaths,
    line: String,
    ack: SyncSender<Result<(), ObservabilityError>>,
}

#[derive(Debug)]
pub(crate) struct CorrelatedErrorSink {
    tx: SyncSender<SinkWriteRequest>,
    ack_timeout: Duration,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct SinkWorkerControl {
    received: std::sync::Arc<std::sync::Barrier>,
    release: std::sync::Arc<std::sync::Barrier>,
}

#[cfg(test)]
impl SinkWorkerControl {
    fn new() -> Self {
        Self {
            received: std::sync::Arc::new(std::sync::Barrier::new(2)),
            release: std::sync::Arc::new(std::sync::Barrier::new(2)),
        }
    }
}

impl CorrelatedErrorSink {
    fn with_capacity(capacity: usize, ack_timeout: Duration, write_delay: Duration) -> Self {
        Self::with_capacity_inner(
            capacity,
            ack_timeout,
            write_delay,
            #[cfg(test)]
            None,
        )
    }

    #[cfg(test)]
    fn with_capacity_for_test(
        capacity: usize,
        ack_timeout: Duration,
        write_delay: Duration,
        control: SinkWorkerControl,
    ) -> Self {
        Self::with_capacity_inner(capacity, ack_timeout, write_delay, Some(control))
    }

    fn with_capacity_inner(
        capacity: usize,
        ack_timeout: Duration,
        write_delay: Duration,
        #[cfg(test)] control: Option<SinkWorkerControl>,
    ) -> Self {
        let (tx, rx) = sync_channel::<SinkWriteRequest>(capacity);
        std::thread::Builder::new()
            .name("tanren-internal-error-sink".to_owned())
            .spawn(move || {
                while let Ok(req) = rx.recv() {
                    #[cfg(test)]
                    if let Some(control) = &control {
                        control.received.wait();
                        control.release.wait();
                    }

                    if !write_delay.is_zero() {
                        std::thread::sleep(write_delay);
                    }

                    let result = sink_write_with_fallback(&req.paths, &req.line);
                    let _ = req.ack.send(result);
                }
            })
            .expect("internal-error sink worker thread must spawn");
        Self { tx, ack_timeout }
    }

    pub(crate) fn emit(&self, paths: &SinkPaths, line: String) -> Result<(), ObservabilityError> {
        let (ack_tx, ack_rx) = sync_channel(1);
        let fallback_paths = paths.clone();
        let fallback_line = line.clone();
        let req = SinkWriteRequest {
            paths: paths.clone(),
            line,
            ack: ack_tx,
        };

        if self.tx.try_send(req).is_err() {
            return persist_to_spool(&fallback_paths, &fallback_line);
        }

        match ack_rx.recv_timeout(self.ack_timeout) {
            Ok(write_result) => write_result,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                persist_to_spool(&fallback_paths, &fallback_line)
            }
        }
    }
}

pub(crate) fn global_correlated_error_sink() -> &'static CorrelatedErrorSink {
    static SINK: OnceLock<CorrelatedErrorSink> = OnceLock::new();
    SINK.get_or_init(|| {
        CorrelatedErrorSink::with_capacity(
            DEFAULT_QUEUE_CAPACITY,
            DEFAULT_ACK_TIMEOUT,
            DEFAULT_WRITE_DELAY,
        )
    })
}

pub(crate) fn default_internal_error_sink_paths() -> Result<SinkPaths, ObservabilityError> {
    let path_override = std::env::var(INTERNAL_ERROR_SINK_PATH_ENV_VAR).ok();
    let xdg_state_home = std::env::var("XDG_STATE_HOME").ok();
    let home = std::env::var("HOME").ok();
    let primary = derive_internal_error_sink_primary_path(
        path_override.as_deref(),
        xdg_state_home.as_deref(),
        home.as_deref(),
    )?;

    Ok(SinkPaths {
        spool: spool_path_for(&primary),
        primary,
    })
}

fn sink_write_with_fallback(paths: &SinkPaths, line: &str) -> Result<(), ObservabilityError> {
    let _ = drain_spool(paths);

    if let Err(primary_err) = append_jsonl_line_with_retry(&paths.primary, line) {
        return append_jsonl_line_with_retry(&paths.spool, line).map_err(|spool_err| {
            ObservabilityError::SinkIo(format!(
                "primary sink write failed ({}): {}; spool fallback failed ({}): {}",
                paths.primary.display(),
                primary_err,
                paths.spool.display(),
                spool_err
            ))
        });
    }

    let _ = drain_spool(paths);
    Ok(())
}

fn persist_to_spool(paths: &SinkPaths, line: &str) -> Result<(), ObservabilityError> {
    append_jsonl_line_with_retry(&paths.spool, line)
}

fn append_jsonl_line_with_retry(path: &Path, line: &str) -> Result<(), ObservabilityError> {
    let mut last_err: Option<ObservabilityError> = None;
    for attempt in 0..=RETRY_ATTEMPTS {
        match append_jsonl_line(path, line) {
            Ok(()) => return Ok(()),
            Err(err) => {
                last_err = Some(err);
                if attempt < RETRY_ATTEMPTS {
                    std::thread::sleep(RETRY_BACKOFF);
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        ObservabilityError::SinkIo(format!("failed to write {} after retries", path.display()))
    }))
}

fn drain_spool(paths: &SinkPaths) -> Result<(), ObservabilityError> {
    let draining_path = draining_path_for(&paths.spool);
    match std::fs::rename(&paths.spool, &draining_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(ObservabilityError::SinkIo(format!(
                "rename {} -> {}: {err}",
                paths.spool.display(),
                draining_path.display()
            )));
        }
    }

    let flush_result = flush_draining_to_primary(&draining_path, &paths.primary);
    if flush_result.is_err() {
        let _ = recover_draining_back_to_spool(&draining_path, &paths.spool);
    }

    let _ = std::fs::remove_file(&draining_path);
    flush_result
}

fn flush_draining_to_primary(draining: &Path, primary: &Path) -> Result<(), ObservabilityError> {
    let file = std::fs::File::open(draining)
        .map_err(|err| ObservabilityError::SinkIo(format!("open {}: {err}", draining.display())))?;
    let reader = std::io::BufReader::new(file);

    for line in reader.lines() {
        let line = line.map_err(|err| {
            ObservabilityError::SinkIo(format!("read {}: {err}", draining.display()))
        })?;
        append_jsonl_line_with_retry(primary, &line)?;
    }

    Ok(())
}

fn recover_draining_back_to_spool(draining: &Path, spool: &Path) -> Result<(), ObservabilityError> {
    let file = std::fs::File::open(draining)
        .map_err(|err| ObservabilityError::SinkIo(format!("open {}: {err}", draining.display())))?;
    let reader = std::io::BufReader::new(file);

    for line in reader.lines() {
        let line = line.map_err(|err| {
            ObservabilityError::SinkIo(format!("read {}: {err}", draining.display()))
        })?;
        append_jsonl_line_with_retry(spool, &line)?;
    }

    Ok(())
}

pub(crate) fn derive_internal_error_sink_primary_path(
    path_override: Option<&str>,
    xdg_state_home: Option<&str>,
    home: Option<&str>,
) -> Result<PathBuf, ObservabilityError> {
    if let Some(path_override) = path_override
        && !path_override.trim().is_empty()
    {
        return Ok(PathBuf::from(path_override.trim()));
    }

    if let Some(xdg_state_home) = xdg_state_home
        && !xdg_state_home.trim().is_empty()
    {
        return Ok(PathBuf::from(xdg_state_home)
            .join("tanren")
            .join(SINK_FILENAME));
    }

    if let Some(home) = home
        && !home.trim().is_empty()
    {
        return Ok(PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("tanren")
            .join(SINK_FILENAME));
    }

    Err(ObservabilityError::SinkIo(format!(
        "internal error sink path is undefined; set {INTERNAL_ERROR_SINK_PATH_ENV_VAR} or XDG_STATE_HOME or HOME"
    )))
}

fn spool_path_for(primary: &Path) -> PathBuf {
    let file_name = primary
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or(SINK_FILENAME);
    primary.with_file_name(format!("{file_name}{SPOOL_SUFFIX}"))
}

fn draining_path_for(spool: &Path) -> PathBuf {
    let file_name = spool
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("internal-errors.jsonl.spool");
    spool.with_file_name(format!("{file_name}{DRAINING_SUFFIX}"))
}

pub(crate) fn append_jsonl_line(path: &Path, line: &str) -> Result<(), ObservabilityError> {
    let parent = path.parent().ok_or_else(|| {
        ObservabilityError::SinkIo(format!("missing parent directory for {}", path.display()))
    })?;
    std::fs::create_dir_all(parent).map_err(|err| {
        ObservabilityError::SinkIo(format!("create_dir_all {}: {err}", parent.display()))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| ObservabilityError::SinkIo(format!("open {}: {err}", path.display())))?;
    writeln!(file, "{line}")
        .map_err(|err| ObservabilityError::SinkIo(format!("write {}: {err}", path.display())))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use super::{
        CorrelatedErrorSink, SinkPaths, SinkWorkerControl, derive_internal_error_sink_primary_path,
    };
    use uuid::Uuid;

    fn temp_sink_paths() -> SinkPaths {
        let base = std::env::temp_dir().join(format!("tanren-observability-{}", Uuid::now_v7()));
        SinkPaths {
            primary: base.with_extension("jsonl"),
            spool: base.with_extension("jsonl.spool"),
        }
    }

    fn cleanup(paths: &SinkPaths) {
        let _ = std::fs::remove_file(&paths.primary);
        let _ = std::fs::remove_file(&paths.spool);
        let _ = std::fs::remove_file(paths.spool.with_extension("spool.draining"));
    }

    #[test]
    fn derive_sink_path_prefers_explicit_override() {
        let path = derive_internal_error_sink_primary_path(
            Some("/tmp/custom-sink.jsonl"),
            Some("/xdg"),
            Some("/home"),
        )
        .expect("path");
        assert_eq!(path, PathBuf::from("/tmp/custom-sink.jsonl"));
    }

    #[test]
    fn derive_sink_path_uses_xdg_state_home() {
        let path = derive_internal_error_sink_primary_path(None, Some("/xdg"), Some("/home"))
            .expect("path");
        assert_eq!(path, PathBuf::from("/xdg/tanren/internal-errors.jsonl"));
    }

    #[test]
    fn derive_sink_path_falls_back_to_home_state() {
        let path =
            derive_internal_error_sink_primary_path(None, None, Some("/home/alice")).expect("path");
        assert_eq!(
            path,
            PathBuf::from("/home/alice/.local/state/tanren/internal-errors.jsonl")
        );
    }

    #[test]
    fn derive_sink_path_fails_closed_when_no_base_dir_exists() {
        let err = derive_internal_error_sink_primary_path(None, None, None).expect_err("must fail");
        assert!(matches!(
            err,
            crate::ObservabilityError::SinkIo(msg) if msg.contains("undefined")
        ));
    }

    #[test]
    fn correlated_error_sink_uses_spool_on_timeout() {
        let paths = temp_sink_paths();
        cleanup(&paths);
        let sink = CorrelatedErrorSink::with_capacity(
            1,
            Duration::from_millis(5),
            Duration::from_millis(50),
        );

        sink.emit(&paths, "{\"timed_out\":1}".to_owned())
            .expect("timeout fallback should spool");

        let spool = std::fs::read_to_string(&paths.spool).expect("spool contents");
        assert!(spool.contains("timed_out"));
        cleanup(&paths);
    }

    #[test]
    fn correlated_error_sink_uses_spool_when_queue_saturated() {
        let control = SinkWorkerControl::new();
        let sink = Arc::new(CorrelatedErrorSink::with_capacity_for_test(
            0,
            Duration::from_secs(1),
            Duration::ZERO,
            control.clone(),
        ));
        let paths = temp_sink_paths();
        cleanup(&paths);

        let first_sink = Arc::clone(&sink);
        let first_paths = paths.clone();
        let first =
            std::thread::spawn(move || first_sink.emit(&first_paths, "{\"first\":1}".to_owned()));
        control.received.wait();

        sink.emit(&paths, "{\"second\":2}".to_owned())
            .expect("queue saturation should fallback to spool");
        let spool = std::fs::read_to_string(&paths.spool).expect("spool contents");
        assert!(spool.contains("second"));

        control.release.wait();
        let first_result = first.join().expect("first sender thread should join");
        assert!(first_result.is_ok(), "first sender should complete");
        cleanup(&paths);
    }

    #[test]
    fn correlated_error_sink_drains_spool_into_primary() {
        let paths = temp_sink_paths();
        cleanup(&paths);
        std::fs::create_dir_all(paths.spool.parent().expect("parent")).expect("mkdir");
        std::fs::write(&paths.spool, "{\"spooled\":1}\n").expect("seed spool");

        let sink = CorrelatedErrorSink::with_capacity(1, Duration::from_secs(1), Duration::ZERO);
        sink.emit(&paths, "{\"fresh\":1}".to_owned())
            .expect("emit should succeed");

        let primary = std::fs::read_to_string(&paths.primary).expect("primary contents");
        assert!(primary.contains("spooled"));
        assert!(primary.contains("fresh"));
        cleanup(&paths);
    }
}
