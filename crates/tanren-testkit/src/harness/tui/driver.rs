use std::path::Path;
use std::process::Command;
use std::time::Duration;

use expectrl::process::Healthcheck;
use expectrl::{Any, Captures, Eof, Session};
use portable_pty::native_pty_system;
use tanren_identity_policy::AccountId;

use crate::harness::{HarnessError, HarnessResult};

const SWITCH_OVERRIDE_ENV: &str = "TANREN_TUI_TEST_SWITCH_TARGET_ACCOUNT_ID";
const KEY_ENTER: &[u8] = b"\r";
const KEY_TAB: &[u8] = b"\t";
const KEY_Q: &[u8] = b"q";
const KEY_ESC: &[u8] = b"\x1b";
const KEY_CTRL_C: &[u8] = b"\x03";

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
    for _ in 0..index {
        session
            .send(KEY_TAB)
            .map_err(|e| HarnessError::Transport(format!("send down: {e}")))?;
    }
    session
        .send(KEY_ENTER)
        .map_err(|e| HarnessError::Transport(format!("send enter: {e}")))?;
    Ok(())
}

pub(super) fn send_form_fields(session: &mut Session, fields: &[&str]) -> HarnessResult<()> {
    for (index, field) in fields.iter().enumerate() {
        session
            .send(field.as_bytes())
            .map_err(|e| HarnessError::Transport(format!("send form field: {e}")))?;
        if index + 1 < fields.len() {
            session
                .send(KEY_TAB)
                .map_err(|e| HarnessError::Transport(format!("send tab: {e}")))?;
        }
    }
    session
        .send(KEY_ENTER)
        .map_err(|e| HarnessError::Transport(format!("send enter: {e}")))?;
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
        .map_err(|e| HarnessError::Transport(format!("send down: {e}")))
}

pub(super) fn expect_any_text(
    session: &mut Session,
    needles: &[&str],
    output: &mut String,
) -> HarnessResult<String> {
    let captures = session
        .expect(Any(needles))
        .map_err(|e| HarnessError::Transport(format!("expect any {needles:?}: {e}")))?;
    let chunk = append_capture(&captures, output);
    for needle in needles {
        if chunk.contains(needle) {
            return Ok((*needle).to_owned());
        }
    }
    Err(HarnessError::Transport(format!(
        "matched output did not contain any expected needle; looked for {needles:?}"
    )))
}

pub(super) fn expect_text(
    session: &mut Session,
    needle: &str,
    output: &mut String,
) -> HarnessResult<()> {
    let captures = session
        .expect(needle)
        .map_err(|e| HarnessError::Transport(format!("expect `{needle}`: {e}")))?;
    let _ = append_capture(&captures, output);
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
    chunk
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
        .env("TANREN_WINDOW_ID", window_id);
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
    session.set_expect_timeout(Some(Duration::from_secs(10)));
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
    match regex::Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]") {
        Ok(ansi) => ansi.replace_all(output, "").into_owned(),
        Err(_) => output.to_owned(),
    }
}
