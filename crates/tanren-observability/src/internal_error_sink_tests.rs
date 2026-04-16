//! Tests for the correlated internal error sink.
//!
//! Extracted from `internal_error_sink.rs` into a sibling module file
//! so the implementation stays under the project's 500-line-per-file
//! ceiling. The module is wired in via `#[path]` at the bottom of
//! `internal_error_sink.rs` and shares the same `super::` namespace.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use uuid::Uuid;

use super::{
    CorrelatedErrorSink, SPILL_DROP_COUNT, SinkPaths, SinkWorkerControl, SpillWorkerControl,
    decide_safe_to_remove_draining, derive_internal_error_sink_primary_path, spill_drop_count,
};

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
    let path =
        derive_internal_error_sink_primary_path(None, Some("/xdg"), Some("/home")).expect("path");
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
fn safe_to_remove_draining_when_flush_succeeds() {
    let recover_called = std::cell::Cell::new(false);
    let safe = decide_safe_to_remove_draining(&Ok(()), || {
        recover_called.set(true);
        Ok(())
    });
    assert!(safe, "successful flush must allow removal");
    assert!(
        !recover_called.get(),
        "successful flush must not invoke recovery"
    );
}

#[test]
fn safe_to_remove_draining_when_flush_fails_but_recovery_succeeds() {
    let safe = decide_safe_to_remove_draining(
        &Err(crate::ObservabilityError::SinkIo("primary down".to_owned())),
        || Ok(()),
    );
    assert!(safe, "recovery to spool guarantees data is durable");
}

#[test]
fn unsafe_to_remove_draining_when_both_flush_and_recovery_fail() {
    // Regression: the prior implementation unconditionally deleted
    // the `.draining` file, silently discarding records when both
    // primary and spool were unwritable. The decision helper must
    // refuse removal so the next drain cycle can retry.
    let safe = decide_safe_to_remove_draining(
        &Err(crate::ObservabilityError::SinkIo("primary down".to_owned())),
        || Err(crate::ObservabilityError::SinkIo("spool down".to_owned())),
    );
    assert!(
        !safe,
        "must preserve draining file when both flush and recovery fail"
    );
}

fn wait_until_spool_contains(path: &std::path::Path, needle: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Ok(contents) = std::fs::read_to_string(path)
            && contents.contains(needle)
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

#[test]
fn correlated_error_sink_uses_spool_on_timeout() {
    let paths = temp_sink_paths();
    cleanup(&paths);
    let sink =
        CorrelatedErrorSink::with_capacity(1, Duration::from_millis(5), Duration::from_millis(50));

    sink.emit(&paths, "{\"timed_out\":1}".to_owned())
        .expect("timeout fallback should spool");

    assert!(
        wait_until_spool_contains(&paths.spool, "timed_out"),
        "spill worker should eventually write the timed-out line"
    );
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
        .expect("queue saturation should fallback to spill worker");
    assert!(
        wait_until_spool_contains(&paths.spool, "second"),
        "spill worker must drain saturated emit to spool"
    );

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

#[test]
fn emit_returns_quickly_when_primary_is_blocked_and_spill_has_room() {
    // Block the primary worker so the caller's ack_recv_timeout
    // fires. The caller must return promptly via the spill worker
    // rather than blocking on synchronous spool I/O.
    let control = SinkWorkerControl::new();
    let sink = Arc::new(CorrelatedErrorSink::with_capacity_for_test(
        1,
        Duration::from_millis(5),
        Duration::from_millis(200),
        control.clone(),
    ));
    let paths = temp_sink_paths();
    cleanup(&paths);

    // Bootstrap: the first emit will be accepted by the primary
    // worker; we hold it there via the barrier so subsequent sends
    // can experience saturation/timeout.
    let bg_sink = Arc::clone(&sink);
    let bg_paths = paths.clone();
    let bg = std::thread::spawn(move || {
        bg_sink
            .emit(&bg_paths, "{\"bg\":1}".to_owned())
            .expect("bg emit");
    });
    control.received.wait();

    // Now the primary worker is held at the control barrier. Any
    // further emit saturates the primary channel (capacity 1, fully
    // consumed by bg). The caller must offload to the spill worker
    // and return quickly.
    let start = Instant::now();
    sink.emit(&paths, "{\"spilled\":1}".to_owned())
        .expect("spill path must succeed");
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(100),
        "spill offload must not block caller (observed {elapsed:?})"
    );

    control.release.wait();
    bg.join().expect("bg join");
    assert!(
        wait_until_spool_contains(&paths.spool, "spilled"),
        "spill worker must eventually persist the record"
    );
    cleanup(&paths);
}

#[test]
fn spill_drop_count_increments_when_spill_queue_exhausted() {
    // Deterministic saturation of both queues. Trace:
    // - Primary & spill capacity 1 each; both workers held by
    //   control barriers so they pull once and then park.
    // - emit #1: primary accepts + worker pulls and blocks on
    //   `received`; caller's ack-timeout fires (5ms); spill accepts
    //   + spill worker pulls and blocks on `received`.
    // - We then meet the spill worker at `received`, letting it move
    //   to `release` (still parked).
    // - emit #2: primary slot refills (worker still blocked);
    //   ack-timeout fires; spill slot refills (worker still at
    //   `release`).
    // - emit #3: primary slot is full → fall through to spill; spill
    //   slot is full → `try_send` fails, counter ++.
    let primary_control = SinkWorkerControl::new();
    let spill_control = SpillWorkerControl::new();
    let sink = Arc::new(CorrelatedErrorSink::with_full_test_controls(
        1,
        1,
        Duration::from_millis(5),
        Duration::ZERO,
        Some(primary_control.clone()),
        Some(spill_control.clone()),
    ));
    let paths = temp_sink_paths();
    cleanup(&paths);

    let baseline = SPILL_DROP_COUNT.load(Ordering::Relaxed);

    sink.emit(&paths, "{\"first\":1}".to_owned())
        .expect("first emit must route through spill");
    // Wait for spill worker to have pulled the first record and
    // parked at `received`. Meeting it lets it advance to `release`
    // (still parked).
    spill_control.received.wait();

    sink.emit(&paths, "{\"second\":2}".to_owned())
        .expect("second emit must fill the spill slot");

    let err = sink
        .emit(&paths, "{\"third\":3}".to_owned())
        .expect_err("third emit must hit exhaustion");
    assert!(
        matches!(
            &err,
            crate::ObservabilityError::SinkIo(msg)
                if msg.contains("spill queue exhausted")
        ),
        "expected SinkIo(spill queue exhausted), got {err:?}"
    );
    assert!(
        SPILL_DROP_COUNT.load(Ordering::Relaxed) > baseline,
        "drop counter must advance on exhaustion"
    );
    assert!(spill_drop_count() >= 1, "public accessor should see drops");

    // Unblock workers so the test exits cleanly; other tests in this
    // file re-use `spill_drop_count` so we must not leak threads that
    // could interfere with subsequent asserts.
    spill_control.release.wait();
    // Primary worker was parked at `received`; meet it and then at
    // `release` so its thread can exit.
    primary_control.received.wait();
    primary_control.release.wait();
    cleanup(&paths);
}
