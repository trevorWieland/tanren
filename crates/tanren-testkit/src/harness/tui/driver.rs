//! TUI PTY driver — ANSI-aware expect matching for ratatui rendering.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use expectrl::process::Healthcheck;
use expectrl::{Captures, Eof, Regex, Session};
use portable_pty::native_pty_system;
use regex::Regex as StdRegex;
use tanren_identity_policy::AccountId;

use crate::harness::{HarnessError, HarnessResult};

const SWITCH_OVERRIDE_ENV: &str = "TANREN_TUI_TEST_SWITCH_TARGET_ACCOUNT_ID";
const KEY_ENTER: &[u8] = b"\r";
const KEY_TAB: &[u8] = b"\t";
const KEY_Q: &[u8] = b"q";
const KEY_ESC: &[u8] = b"\x1b";
const KEY_CTRL_C: &[u8] = b"\x03";

/// ANSI escape sequence pattern — matches CSI sequences, OSC sequences,
/// and other common terminal control sequences that ratatui/crossterm
/// emits between text characters.
const ANSI_RE: &str = r"\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07|\x1b\([B0UK]|\x1b[>=<]";

/// Build a regex pattern that matches `text` with zero or more ANSI escape
/// sequences allowed between any two characters. This handles the fact that
/// ratatui/crossterm emits cursor-positioning and styling escape codes
/// interspersed within rendered text.
fn ansi_aware_pattern(text: &str) -> String {
    let ansi_gap = format!("(?:{ANSI_RE})*");
    let mut pattern = String::with_capacity(text.len() * ansi_gap.len() * 2);
    for (i, ch) in text.chars().enumerate() {
        if i > 0 {
            pattern.push_str(&ansi_gap);
        }
        if ch == ' ' {
            // Spaces may be collapsed or replaced by cursor positioning,
            // so match one or more ANSI sequences plus optional whitespace.
            pattern.push_str(r"\s*");
            pattern.push_str(&ansi_gap);
        } else {
            // Escape regex-special characters
            match ch {
                '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '\\'
                | '|' => {
                    pattern.push('\\');
                    pattern.push(ch);
                }
                _ => {
                    pattern.push(ch);
                }
            }
        }
    }
    pattern
}

pub(super) fn run_tui_script<T, F>(
    binary: &Path,
    db_url: &str,
    session_file: &Path,
    window_id: &str,
    override_target: Option<AccountId>,
    operation: &'static str,
    script: F,
) -> HarnessResult<T>
where
    F: FnOnce(&mut Session, &mut String) -> HarnessResult<T>,
{
    let mut session = spawn_session(binary, db_url, session_file, window_id, override_target)?;
    let mut output = String::new();
    expect_text(&mut session, "Choose an action:", &mut output)?;
    let result = script(&mut session, &mut output);
    let teardown = shutdown_session(&mut session);

    let output = normalize_tui_output(&output);
    tracing::info!(target: "tanren_testkit::tui", operation, output = %output);
    if let Err(err) = teardown {
        tracing::warn!(
            target: "tanren_testkit::tui",
            operation,
            error = %err,
            "failed to cleanly terminate tanren-tui pty session"
        );
        if result.is_ok() {
            return Err(err);
        }
    }
    result
}

pub(super) fn select_menu_index(session: &mut Session, index: usize) -> HarnessResult<()> {
    tracing::debug!(target: "tanren_testkit::tui", index, "select_menu_index");
    for _ in 0..index {
        session
            .send(KEY_TAB)
            .map_err(|e| HarnessError::Transport(format!("send down: {e}")))?;
        std::thread::sleep(FIELD_DELAY);
    }
    session
        .send(KEY_ENTER)
        .map_err(|e| HarnessError::Transport(format!("send enter: {e}")))?;
    Ok(())
}

/// Inter-field delay to let ratatui settle after a focus-change (TAB) redraw.
const FIELD_DELAY: Duration = Duration::from_millis(10);

pub(super) fn send_form_fields(session: &mut Session, fields: &[&str]) -> HarnessResult<()> {
    tracing::debug!(target: "tanren_testkit::tui", field_count = fields.len(), "send_form_fields start");
    for (index, field) in fields.iter().enumerate() {
        tracing::debug!(target: "tanren_testkit::tui", index, len = field.len(), "typing field");
        type_field(session, field)?;
        if index + 1 < fields.len() {
            session
                .send(KEY_TAB)
                .map_err(|e| HarnessError::Transport(format!("send tab: {e}")))?;
            std::thread::sleep(FIELD_DELAY);
        }
    }
    session
        .send(KEY_ENTER)
        .map_err(|e| HarnessError::Transport(format!("send enter: {e}")))?;
    tracing::debug!(target: "tanren_testkit::tui", "send_form_fields done");
    // Brief pause to let the TUI process the submit, then check liveness.
    // Do NOT drain the PTY output buffer here — the subsequent `expect`
    // call must be able to read the TUI's outcome screen. The drain
    // previously consumed the response before expect could match it,
    // causing timeouts on "account_id:" / error-code patterns.
    std::thread::sleep(Duration::from_millis(300));
    let alive = session.get_process_mut().is_alive().unwrap_or(false);
    tracing::debug!(target: "tanren_testkit::tui", alive, "tui liveness after form submit");
    if !alive {
        return Err(HarnessError::Transport(
            "tanren-tui exited after form submit".to_owned(),
        ));
    }
    Ok(())
}

/// Type a field value by sending the entire string at once.
/// The TUI processes one key event per event-loop iteration and
/// re-draws the frame after each. Sending the whole field in a
/// single write avoids per-character PTY output bursts that can
/// fill the kernel buffer and deadlock the TUI.
fn type_field(session: &mut Session, value: &str) -> HarnessResult<()> {
    session
        .send(value.as_bytes())
        .map_err(|e| HarnessError::Transport(format!("send form field: {e}")))?;
    Ok(())
}

pub(super) fn send_enter(session: &mut Session) -> HarnessResult<()> {
    session
        .send(KEY_ENTER)
        .map_err(|e| HarnessError::Transport(format!("send enter: {e}")))
}

pub(super) fn send_down(session: &mut Session) -> HarnessResult<()> {
    session
        .send(KEY_TAB)
        .map_err(|e| HarnessError::Transport(format!("send down: {e}")))?;
    std::thread::sleep(FIELD_DELAY);
    Ok(())
}

/// Wait for any of the given text fragments to appear in the PTY output,
/// accounting for ANSI escape sequences between characters.
pub(super) fn expect_any_text(
    session: &mut Session,
    needles: &[&str],
    output: &mut String,
) -> HarnessResult<String> {
    let patterns: Vec<String> = needles.iter().map(|n| ansi_aware_pattern(n)).collect();
    // Build a combined regex that matches any of the patterns.
    let combined = patterns
        .iter()
        .map(|p| format!("({p})"))
        .collect::<Vec<_>>()
        .join("|");
    let captures = session
        .expect(Regex(&combined))
        .map_err(|e| HarnessError::Transport(format!("expect any {needles:?}: {e}")))?;
    let chunk = append_capture(&captures, output);
    let stripped = strip_ansi(&chunk);
    for needle in needles {
        if stripped.contains(needle) {
            return Ok((*needle).to_owned());
        }
    }
    Err(HarnessError::Transport(format!(
        "matched output did not contain any expected needle; looked for {needles:?}"
    )))
}

/// Wait for a specific text fragment to appear in the PTY output,
/// accounting for ANSI escape sequences that ratatui/crossterm
/// emits between rendered characters.
pub(super) fn expect_text(
    session: &mut Session,
    needle: &str,
    output: &mut String,
) -> HarnessResult<()> {
    let pattern = ansi_aware_pattern(needle);
    tracing::debug!(target: "tanren_testkit::tui", needle, "expect_text start");
    let captures = session.expect(Regex(&pattern)).map_err(|e| {
        tracing::warn!(target: "tanren_testkit::tui", needle, error = %e, "expect_text failed");
        HarnessError::Transport(format!("expect `{needle}`: {e}"))
    })?;
    let chunk = append_capture(&captures, output);
    let stripped = strip_ansi(&chunk);
    tracing::debug!(target: "tanren_testkit::tui", needle, matched_len = chunk.len(), stripped_preview = %stripped.chars().take(80).collect::<String>(), "expect_text matched");
    Ok(())
}

fn append_capture(captures: &Captures, output: &mut String) -> String {
    let chunk = captures
        .get(0)
        .map(|matched| String::from_utf8_lossy(matched).into_owned())
        .unwrap_or_default();
    if !chunk.is_empty() {
        output.push_str(&chunk);
        output.push('\n');
    }
    let stripped = strip_ansi(&chunk);
    if !stripped.is_empty() {
        tracing::debug!(
            target: "tanren_testkit::tui",
            raw_len = chunk.len(),
            stripped_preview = %stripped.chars().take(120).collect::<String>(),
            "append_capture"
        );
    }
    chunk
}

/// Strip ANSI escape sequences from a string.
fn strip_ansi(text: &str) -> String {
    static RE: std::sync::OnceLock<StdRegex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| StdRegex::new(ANSI_RE).expect("ANSI regex must compile"));
    re.replace_all(text, "").into_owned()
}

fn spawn_session(
    binary: &Path,
    db_url: &str,
    session_file: &Path,
    window_id: &str,
    override_target: Option<AccountId>,
) -> HarnessResult<Session> {
    // Ensure the platform PTY backend is available before spawning.
    let pty_system = native_pty_system();
    drop(pty_system);

    let mut cmd = Command::new(binary);
    cmd.env("DATABASE_URL", db_url)
        .env("TANREN_SESSION_FILE", session_file)
        .env("TANREN_WINDOW_ID", window_id)
        // Suppress sqlx/tracing log output that would corrupt the PTY
        // rendering stream. The TUI binary uses tracing; without this,
        // sqlx query logs appear in the PTY output mixed with the
        // ratatui rendering, breaking ANSI-aware text matching.
        .env("RUST_LOG", "error");
    if let Some(target) = override_target {
        cmd.env(SWITCH_OVERRIDE_ENV, target.to_string());
    }

    let mut session = Session::spawn(cmd)
        .map_err(|e| HarnessError::Transport(format!("spawn tanren-tui in pty: {e}")))?;
    // Fail fast: give the TUI binary a moment to start, then verify
    // it is still alive. If the binary exits immediately (e.g. raw-mode
    // setup failure due to missing /dev/tty), the PTY session will
    // report it as not-alive within milliseconds instead of hanging for
    // a 60-second expect timeout.
    std::thread::sleep(Duration::from_millis(250));
    if !session
        .get_process_mut()
        .is_alive()
        .map_err(|e| HarnessError::Transport(format!("check tanren-tui liveness: {e}")))?
    {
        return Err(HarnessError::Transport(
            "tanren-tui exited immediately — PTY or terminal setup unavailable".to_owned(),
        ));
    }
    session.set_expect_timeout(Some(Duration::from_secs(20)));
    Ok(session)
}

fn shutdown_session(session: &mut Session) -> HarnessResult<()> {
    // Prefer a normal UI exit path first.
    let _ = session.send(KEY_ESC);
    let _ = session.send(KEY_Q);
    let _ = session.send(KEY_Q);
    session.set_expect_timeout(Some(Duration::from_secs(1)));
    if session.expect(Eof).is_ok() {
        reap_session(session);
        return Ok(());
    }

    // Escalate to Ctrl-C before killing the process directly.
    let _ = session.send(KEY_CTRL_C);
    if session.expect(Eof).is_ok() {
        reap_session(session);
        return Ok(());
    }

    #[cfg(unix)]
    {
        session
            .get_process_mut()
            .exit(true)
            .map_err(|e| HarnessError::Transport(format!("force-exit tanren-tui pty: {e}")))?;
        reap_session(session);
        Ok(())
    }

    #[cfg(not(unix))]
    {
        Err(HarnessError::Transport(
            "tanren-tui pty session did not exit cleanly".to_owned(),
        ))
    }
}

fn reap_session(session: &mut Session) {
    #[cfg(unix)]
    {
        let _ = session.get_process_mut().wait();
    }
}

fn normalize_tui_output(output: &str) -> String {
    tracing::debug!(target: "tanren_testkit::tui", len = output.len(), "raw PTY output");
    strip_ansi(output)
}
