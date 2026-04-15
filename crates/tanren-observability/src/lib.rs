//! Shared telemetry primitives for the tanren workspace.
//!
//! # Responsibilities
//!
//! - Tracing subscriber initialization for binary crates
//! - Structured logging with configurable filter levels
//! - Foundation for future OpenTelemetry integration (Lane 0.5+)
//!
//! # Design Rules
//!
//! - No crate emits unstructured logs without correlation context
//! - All telemetry uses structured tracing, never `println!` or `eprintln!`
//! - Binary crates call [`init_tracing`] or [`init_tracing_for_contract_io`] once at startup

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{RecvTimeoutError, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

use chrono::Utc;
use serde::Serialize;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

/// Errors that can occur during observability initialization.
#[derive(Debug, thiserror::Error)]
pub enum ObservabilityError {
    /// The log filter string could not be parsed.
    #[error("failed to parse log filter: {0}")]
    FilterParse(String),

    /// The tracing subscriber has already been initialized.
    #[error("tracing subscriber already initialized")]
    AlreadyInitialized,

    /// A correlated error event could not be serialized.
    #[error("failed to serialize correlated error event: {0}")]
    SinkSerialize(String),

    /// A correlated error event could not be written.
    #[error("failed to write correlated error event: {0}")]
    SinkIo(String),
}

#[derive(Debug)]
struct SinkWriteRequest {
    path: PathBuf,
    line: String,
    ack: SyncSender<Result<(), ObservabilityError>>,
}

#[derive(Debug)]
struct CorrelatedErrorSink {
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
                    let result = append_jsonl_line(&req.path, &req.line);
                    let _ = req.ack.send(result);
                }
            })
            .expect("internal-error sink worker thread must spawn");
        Self { tx, ack_timeout }
    }

    fn emit(&self, path: &Path, line: String) -> Result<(), ObservabilityError> {
        let (ack_tx, ack_rx) = sync_channel(1);
        let req = SinkWriteRequest {
            path: path.to_path_buf(),
            line,
            ack: ack_tx,
        };

        self.tx.try_send(req).map_err(|err| match err {
            TrySendError::Full(_) => {
                ObservabilityError::SinkIo("internal error sink queue saturated".to_owned())
            }
            TrySendError::Disconnected(_) => {
                ObservabilityError::SinkIo("internal error sink worker unavailable".to_owned())
            }
        })?;

        match ack_rx.recv_timeout(self.ack_timeout) {
            Ok(write_result) => write_result,
            Err(RecvTimeoutError::Timeout) => Err(ObservabilityError::SinkIo(
                "internal error sink write timed out".to_owned(),
            )),
            Err(RecvTimeoutError::Disconnected) => Err(ObservabilityError::SinkIo(
                "internal error sink worker unavailable".to_owned(),
            )),
        }
    }
}

fn global_correlated_error_sink() -> &'static CorrelatedErrorSink {
    static SINK: OnceLock<CorrelatedErrorSink> = OnceLock::new();
    SINK.get_or_init(|| {
        CorrelatedErrorSink::with_capacity(512, Duration::from_millis(75), Duration::ZERO)
    })
}

/// Initialize the global tracing subscriber with the given filter level.
///
/// Call once at binary startup. Uses compact human-readable format
/// suitable for CLI and development use. Future lanes will add JSON
/// format for daemon/production use.
///
/// The `level` parameter accepts any valid [`EnvFilter`] directive:
/// - Simple levels: `"info"`, `"debug"`, `"warn"`
/// - Targeted: `"tanren=debug,sea_orm=warn"`
///
/// # Errors
///
/// Returns [`ObservabilityError::FilterParse`] if the level string is
/// invalid, or [`ObservabilityError::AlreadyInitialized`] if the global
/// subscriber was already set.
pub fn init_tracing(level: &str) -> Result<(), ObservabilityError> {
    let filter =
        EnvFilter::try_new(level).map_err(|e| ObservabilityError::FilterParse(e.to_string()))?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .try_init()
        .map_err(|_| ObservabilityError::AlreadyInitialized)
}

/// Initialize tracing for binaries with strict machine I/O contracts.
///
/// This variant validates and installs a global subscriber but writes
/// trace output to a sink so command stdout/stderr remain contract-only.
pub fn init_tracing_for_contract_io(level: &str) -> Result<(), ObservabilityError> {
    let filter =
        EnvFilter::try_new(level).map_err(|e| ObservabilityError::FilterParse(e.to_string()))?;

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_writer(std::io::sink)
        .compact()
        .try_init()
        .map_err(|_| ObservabilityError::AlreadyInitialized)
}

/// Emit an internal error event to the default JSONL sink.
///
/// This is best-effort telemetry for correlation IDs returned in wire
/// responses. It never writes to stdout/stderr.
pub fn emit_correlated_internal_error(
    component: &str,
    error_code: &str,
    correlation_id: Uuid,
    raw_error: &str,
) -> Result<(), ObservabilityError> {
    let path = default_internal_error_sink_path();
    let line = build_correlated_internal_error_jsonl_line(
        component,
        error_code,
        correlation_id,
        raw_error,
    )?;
    global_correlated_error_sink().emit(&path, line)
}

/// Sanitize error text before structured logging.
///
/// This redacts URL userinfo segments (`scheme://user:pass@host`) and
/// common credential-like query parameters to reduce accidental secret
/// leakage in logs.
#[must_use]
pub fn sanitize_error_for_log(raw: &str) -> String {
    let redacted_url_userinfo = redact_url_userinfo(raw);
    redact_query_credentials(&redacted_url_userinfo)
}

fn redact_url_userinfo(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;

    while let Some(rel) = raw[i..].find("://") {
        let scheme_sep = i + rel;
        out.push_str(&raw[i..scheme_sep + 3]);
        let authority_start = scheme_sep + 3;
        let authority_end = raw[authority_start..]
            .find(['/', '?', '#', ' '])
            .map_or(raw.len(), |idx| authority_start + idx);
        let authority = &raw[authority_start..authority_end];

        if let Some(at_idx) = authority.rfind('@') {
            let host = &authority[at_idx + 1..];
            out.push_str("REDACTED@");
            out.push_str(host);
        } else {
            out.push_str(authority);
        }

        i = authority_end;
    }

    out.push_str(&raw[i..]);
    out
}

fn redact_query_credentials(raw: &str) -> String {
    let mut sanitized = raw.to_owned();
    for key in [
        "password", "passwd", "pwd", "token", "api_key", "apikey", "secret",
    ] {
        for prefix in [format!("{key}="), format!("{key}:"), format!("{key}%3d")] {
            sanitized = redact_after_prefix(&sanitized, &prefix);
            sanitized = redact_after_prefix(&sanitized, &prefix.to_ascii_uppercase());
        }
    }
    sanitized
}

fn redact_after_prefix(input: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut start = 0;

    while let Some(idx) = input[start..].find(prefix) {
        let absolute = start + idx;
        let value_start = absolute + prefix.len();
        out.push_str(&input[start..value_start]);
        let value_end = input[value_start..]
            .find(['&', ' ', ';', ',', '"', '\'', ')', ']', '}'])
            .map_or(input.len(), |end| value_start + end);
        out.push_str("REDACTED");
        start = value_end;
    }

    out.push_str(&input[start..]);
    out
}

fn default_internal_error_sink_path() -> PathBuf {
    if let Ok(xdg_state_home) = std::env::var("XDG_STATE_HOME")
        && !xdg_state_home.trim().is_empty()
    {
        return PathBuf::from(xdg_state_home)
            .join("tanren")
            .join("internal-errors.jsonl");
    }

    if let Ok(home) = std::env::var("HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("tanren")
            .join("internal-errors.jsonl");
    }

    std::env::temp_dir()
        .join("tanren")
        .join("internal-errors.jsonl")
}

#[cfg(test)]
fn emit_correlated_internal_error_to_path(
    path: &Path,
    component: &str,
    error_code: &str,
    correlation_id: Uuid,
    raw_error: &str,
) -> Result<(), ObservabilityError> {
    let line = build_correlated_internal_error_jsonl_line(
        component,
        error_code,
        correlation_id,
        raw_error,
    )?;
    append_jsonl_line(path, &line)
}

fn build_correlated_internal_error_jsonl_line(
    component: &str,
    error_code: &str,
    correlation_id: Uuid,
    raw_error: &str,
) -> Result<String, ObservabilityError> {
    let record = CorrelatedInternalErrorRecord {
        timestamp_utc: Utc::now().to_rfc3339(),
        component,
        error_code,
        correlation_id: correlation_id.to_string(),
        message: sanitize_error_for_log(raw_error),
    };
    serde_json::to_string(&record).map_err(|err| ObservabilityError::SinkSerialize(err.to_string()))
}

fn append_jsonl_line(path: &Path, line: &str) -> Result<(), ObservabilityError> {
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

#[derive(Debug, Serialize)]
struct CorrelatedInternalErrorRecord<'a> {
    timestamp_utc: String,
    component: &'a str,
    error_code: &'a str,
    correlation_id: String,
    message: String,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::{
        CorrelatedErrorSink, ObservabilityError, SinkWorkerControl,
        emit_correlated_internal_error_to_path, sanitize_error_for_log,
    };
    use uuid::Uuid;

    #[test]
    fn sanitize_redacts_postgres_url_userinfo() {
        let raw = "failed to connect postgres://alice:supersecret@localhost:5432/tanren";
        let sanitized = sanitize_error_for_log(raw);
        assert!(!sanitized.contains("alice:supersecret"));
        assert!(sanitized.contains("postgres://REDACTED@localhost:5432/tanren"));
    }

    #[test]
    fn sanitize_redacts_sqlite_url_userinfo_and_query_secret() {
        let raw = "bad url sqlite://user:pass@localhost/tmp/t.db?mode=rwc&token=abc123";
        let sanitized = sanitize_error_for_log(raw);
        assert!(!sanitized.contains("user:pass"));
        assert!(sanitized.contains("sqlite://REDACTED@localhost/tmp/t.db"));
        assert!(!sanitized.contains("abc123"));
        assert!(sanitized.contains("token=REDACTED"));
    }

    #[test]
    fn correlated_error_sink_writes_sanitized_jsonl_record() {
        let path =
            std::env::temp_dir().join(format!("tanren-observability-{}.jsonl", Uuid::now_v7()));
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }

        let correlation_id = Uuid::now_v7();
        emit_correlated_internal_error_to_path(
            &path,
            "tanren_cli",
            "internal",
            correlation_id,
            "failed postgres://alice:secret@localhost:5432/tanren?token=abc123",
        )
        .expect("write sink");

        let line = std::fs::read_to_string(&path).expect("read sink");
        let trimmed = line.trim();
        let value: serde_json::Value = serde_json::from_str(trimmed).expect("json");
        assert_eq!(value["component"], "tanren_cli");
        assert_eq!(value["error_code"], "internal");
        assert_eq!(value["correlation_id"], correlation_id.to_string());
        assert!(value["timestamp_utc"].as_str().is_some());
        let message = value["message"].as_str().expect("message");
        assert!(!message.contains("alice:secret"));
        assert!(!message.contains("abc123"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn correlated_error_sink_times_out_on_slow_worker() {
        let sink = CorrelatedErrorSink::with_capacity(
            1,
            Duration::from_millis(5),
            Duration::from_millis(50),
        );
        let path =
            std::env::temp_dir().join(format!("tanren-observability-{}.jsonl", Uuid::now_v7()));
        let err = sink
            .emit(&path, "{\"k\":\"v\"}".to_owned())
            .expect_err("slow sink must time out");
        assert!(matches!(err, ObservabilityError::SinkIo(msg) if msg.contains("timed out")));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn correlated_error_sink_rejects_when_queue_saturated() {
        let control = SinkWorkerControl::new();
        let sink = Arc::new(CorrelatedErrorSink::with_capacity_for_test(
            0,
            Duration::from_secs(1),
            Duration::ZERO,
            control.clone(),
        ));
        let path =
            std::env::temp_dir().join(format!("tanren-observability-{}.jsonl", Uuid::now_v7()));
        let first_sink = Arc::clone(&sink);
        let first_path = path.clone();
        let first =
            std::thread::spawn(move || first_sink.emit(&first_path, "{\"first\":1}".to_owned()));
        control.received.wait();

        let err = sink
            .emit(&path, "{\"second\":2}".to_owned())
            .expect_err("queue saturation must fail fast");
        assert!(matches!(err, ObservabilityError::SinkIo(msg) if msg.contains("queue saturated")));
        control.release.wait();
        let first_result = first.join().expect("first sender thread should join");
        assert!(
            first_result.is_ok(),
            "first sender should complete once worker is released"
        );
        let _ = std::fs::remove_file(&path);
    }
}
