use std::io::{BufRead as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

use crate::ObservabilityError;

pub const INTERNAL_ERROR_SINK_PATH_ENV_VAR: &str = "TANREN_INTERNAL_ERROR_SINK_PATH";

const DEFAULT_QUEUE_CAPACITY: usize = 512;
const DEFAULT_SPILL_CAPACITY: usize = 4_096;
const DEFAULT_ACK_TIMEOUT: Duration = Duration::from_millis(75);
const DEFAULT_WRITE_DELAY: Duration = Duration::ZERO;
const SINK_FILENAME: &str = "internal-errors.jsonl";
const SPOOL_SUFFIX: &str = ".spool";
const DRAINING_SUFFIX: &str = ".draining";
const RETRY_ATTEMPTS: usize = 3;
const RETRY_BACKOFF: Duration = Duration::from_millis(20);

/// Monotonic count of records dropped because both the primary and
/// spill queues were saturated. Operators sampling this should see
/// zero on healthy systems; any non-zero value is a signal of a
/// pathological error storm.
static SPILL_DROP_COUNT: AtomicU64 = AtomicU64::new(0);

/// Read the spill-drop counter.
///
/// Non-decreasing. Safe to call from any thread; uses `Relaxed`
/// ordering because the counter is an approximate operator signal,
/// not a correctness-critical variable.
#[must_use]
pub fn spill_drop_count() -> u64 {
    SPILL_DROP_COUNT.load(Ordering::Relaxed)
}

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

/// Asynchronous spill request. No ack channel — the caller returns
/// success as soon as the spill queue accepts the request.
#[derive(Debug)]
struct SpillRequest {
    paths: SinkPaths,
    line: String,
}

#[derive(Debug)]
pub(crate) struct CorrelatedErrorSink {
    tx: SyncSender<SinkWriteRequest>,
    spill_tx: SyncSender<SpillRequest>,
    ack_timeout: Duration,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct SinkWorkerControl {
    received: std::sync::Arc<std::sync::Barrier>,
    release: std::sync::Arc<std::sync::Barrier>,
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct SpillWorkerControl {
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

#[cfg(test)]
impl SpillWorkerControl {
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
            DEFAULT_SPILL_CAPACITY,
            ack_timeout,
            write_delay,
            #[cfg(test)]
            None,
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
        Self::with_capacity_inner(
            capacity,
            DEFAULT_SPILL_CAPACITY,
            ack_timeout,
            write_delay,
            Some(control),
            None,
        )
    }

    #[cfg(test)]
    fn with_full_test_controls(
        primary_capacity: usize,
        spill_capacity: usize,
        ack_timeout: Duration,
        write_delay: Duration,
        primary_control: Option<SinkWorkerControl>,
        spill_control: Option<SpillWorkerControl>,
    ) -> Self {
        Self::with_capacity_inner(
            primary_capacity,
            spill_capacity,
            ack_timeout,
            write_delay,
            primary_control,
            spill_control,
        )
    }

    fn with_capacity_inner(
        capacity: usize,
        spill_capacity: usize,
        ack_timeout: Duration,
        write_delay: Duration,
        #[cfg(test)] control: Option<SinkWorkerControl>,
        #[cfg(test)] spill_control: Option<SpillWorkerControl>,
    ) -> Self {
        let (tx, rx) = sync_channel::<SinkWriteRequest>(capacity);
        let (spill_tx, spill_rx) = sync_channel::<SpillRequest>(spill_capacity);

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

        // Dedicated spill worker. Drains the spill channel and writes
        // directly to the spool file — never blocks the caller.
        std::thread::Builder::new()
            .name("tanren-internal-error-spill".to_owned())
            .spawn(move || {
                while let Ok(req) = spill_rx.recv() {
                    #[cfg(test)]
                    if let Some(control) = &spill_control {
                        control.received.wait();
                        control.release.wait();
                    }
                    // Best-effort write to spool; failures are not
                    // surfaced to callers because the caller already
                    // got `Ok(())` when the request was queued. We
                    // still retry so transient filesystem hiccups do
                    // not silently drop records.
                    let _ = append_jsonl_line_with_retry(&req.paths.spool, &req.line);
                }
            })
            .expect("internal-error spill worker thread must spawn");

        Self {
            tx,
            spill_tx,
            ack_timeout,
        }
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

        match self.tx.try_send(req) {
            Ok(()) => {}
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                return self.spill(&fallback_paths, fallback_line);
            }
        }

        match ack_rx.recv_timeout(self.ack_timeout) {
            Ok(write_result) => write_result,
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                self.spill(&fallback_paths, fallback_line)
            }
        }
    }

    /// Hand the write off to the spill worker. Returns immediately
    /// when the spill queue has room; only when the spill queue is
    /// also saturated do we give up and increment
    /// [`SPILL_DROP_COUNT`].
    fn spill(&self, paths: &SinkPaths, line: String) -> Result<(), ObservabilityError> {
        let req = SpillRequest {
            paths: paths.clone(),
            line,
        };
        match self.spill_tx.try_send(req) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                SPILL_DROP_COUNT.fetch_add(1, Ordering::Relaxed);
                Err(ObservabilityError::SinkIo(
                    "spill queue exhausted".to_owned(),
                ))
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
    let safe_to_remove = decide_safe_to_remove_draining(&flush_result, || {
        recover_draining_back_to_spool(&draining_path, &paths.spool)
    });

    if safe_to_remove {
        let _ = std::fs::remove_file(&draining_path);
    }
    flush_result
}

/// The `.draining` file may only be deleted once the records it
/// holds are durably accounted for somewhere else — either:
///
/// - the primary flush succeeded (records are now in primary), OR
/// - the flush failed but recovery copied them back to the spool
///   (the next drain cycle will pick them up).
///
/// If BOTH the flush and the recovery fail, the `.draining` file is
/// the sole surviving copy and must be preserved so the next drain
/// attempt can retry. Deleting it here would silently discard
/// correlated error events, breaking the durability contract
/// callers rely on.
fn decide_safe_to_remove_draining<F>(
    flush_result: &Result<(), ObservabilityError>,
    recover: F,
) -> bool
where
    F: FnOnce() -> Result<(), ObservabilityError>,
{
    match flush_result {
        Ok(()) => true,
        Err(_) => recover().is_ok(),
    }
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
#[path = "internal_error_sink_tests.rs"]
mod tests;
