use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use expectrl::Session;
use expectrl::{Signal, WaitStatus};
use portable_pty::{PtySize, native_pty_system};

use super::HarnessError;
use super::tui_screen::{normalize_for_match, sanitize_terminal_text};

const MENU_PROMPT: &str = "Choose an action";
const MENU_ITEM_COUNT: usize = 8;
const QUIET_TICK_MILLIS: u64 = 30;
const QUIET_TICKS_AFTER_OUTCOME: u8 = 4;
const QUIET_TICKS_WITHOUT_OUTCOME: u8 = 67;
const FORM_HINT: &str = "Enter submit";
const QUIET_TICKS_EMPTY: u8 = 100;

#[derive(Debug, Clone, Copy)]
pub(crate) enum TuiMenuChoice {
    SignUp = 0,
    SignIn = 1,
    AcceptInvitation = 2,
    CreateRole = 3,
    EditRole = 4,
    DeleteRole = 5,
    ApplyRole = 6,
    CheckPermission = 7,
}

#[derive(Debug, Clone)]
pub(crate) struct TuiTranscript {
    pub(crate) text: String,
}

impl TuiTranscript {
    #[must_use]
    pub(crate) fn contains(&self, needle: &str) -> bool {
        self.text.contains(needle)
            || normalize_for_match(&self.text).contains(&normalize_for_match(needle))
    }

    #[must_use]
    pub(crate) fn line_value(&self, key: &str) -> Option<String> {
        let prefix = format!("{key}:");
        let mut offset = self.text.len();
        while let Some(idx) = self.text[..offset].rfind(&prefix) {
            let value_start = idx + prefix.len();
            let tail = &self.text[value_start..];
            let value_end = tail.find(['\n', '\r', '│', '|']).unwrap_or(tail.len());
            let value = tail[..value_end].trim();
            if !value.is_empty() {
                return Some(value.to_owned());
            }
            if idx == 0 {
                break;
            }
            offset = idx;
        }
        None
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TuiDriver {
    binary: PathBuf,
    database_url: String,
    expect_timeout: Duration,
    settle_timeout: Duration,
    pty_size: PtySize,
}

impl TuiDriver {
    #[must_use]
    pub(crate) fn new(binary: PathBuf, database_url: String) -> Self {
        Self {
            binary,
            database_url,
            expect_timeout: Duration::from_secs(30),
            settle_timeout: Duration::from_millis(700),
            pty_size: PtySize {
                rows: 40,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            },
        }
    }

    pub(crate) fn submit_form(
        &self,
        choice: TuiMenuChoice,
        form_title: &str,
        values: &[String],
    ) -> Result<TuiTranscript, HarnessError> {
        self.ensure_pty_available()?;
        let mut session = self.spawn_session()?;
        let mut selected_index = 0_usize;
        let result = self.submit_form_in_session(
            &mut session,
            choice,
            form_title,
            values,
            &mut selected_index,
            true,
            false,
        );
        self.finish_session(&mut session, "submit_form")?;
        let text = result?;
        Ok(TuiTranscript {
            text: sanitize_terminal_text(&text),
        })
    }

    pub(crate) fn submit_form_with_sign_in(
        &self,
        email: &str,
        password: &str,
        choice: TuiMenuChoice,
        form_title: &str,
        values: &[String],
    ) -> Result<TuiTranscript, HarnessError> {
        self.ensure_pty_available()?;
        let mut session = self.spawn_session()?;
        let mut selected_index = 0_usize;
        let sign_in_result = self.submit_form_in_session(
            &mut session,
            TuiMenuChoice::SignIn,
            "Sign in",
            &[email.to_owned(), password.to_owned()],
            &mut selected_index,
            true,
            true,
        );
        let form_result = match sign_in_result {
            Ok(_) => self.submit_form_in_session(
                &mut session,
                choice,
                form_title,
                values,
                &mut selected_index,
                true,
                false,
            ),
            Err(err) => Err(err),
        };
        self.finish_session(&mut session, "submit_form_with_sign_in")?;
        let text = form_result?;

        Ok(TuiTranscript {
            text: sanitize_terminal_text(&text),
        })
    }

    pub(crate) fn probe_startup(&self) -> Result<(), HarnessError> {
        self.ensure_pty_available()?;
        let mut session = self.spawn_session()?;
        std::thread::sleep(Duration::from_millis(150));
        self.finish_session(&mut session, "probe_startup")
    }

    fn finish_session(&self, session: &mut Session, context: &str) -> Result<(), HarnessError> {
        let _ = session.send("\u{3}");
        let _ = session.send("qq");
        let grace_deadline = Instant::now() + Duration::from_millis(400);
        loop {
            match session.get_process_mut().status() {
                Ok(status) => {
                    if process_has_exited(status) {
                        return Ok(());
                    }
                    if Instant::now() >= grace_deadline {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(err) => {
                    if process_is_gone(&err) {
                        return Ok(());
                    }
                    return Err(HarnessError::Transport(format!(
                        "check tui child status while finishing `{context}`: {err}"
                    )));
                }
            }
        }

        let _ = session.get_process_mut().kill(Signal::SIGTERM);
        let force_deadline = Instant::now() + Duration::from_millis(800);
        loop {
            match session.get_process_mut().status() {
                Ok(status) => {
                    if process_has_exited(status) {
                        return Ok(());
                    }
                    if Instant::now() >= force_deadline {
                        let _ = session.get_process_mut().kill(Signal::SIGKILL);
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(err) => {
                    if process_is_gone(&err) {
                        return Ok(());
                    }
                    return Err(HarnessError::Transport(format!(
                        "check tui child status after SIGTERM for `{context}`: {err}"
                    )));
                }
            }
        }

        let reap_deadline = Instant::now() + Duration::from_millis(800);
        loop {
            match session.get_process_mut().status() {
                Ok(status) => {
                    if process_has_exited(status) {
                        return Ok(());
                    }
                    if Instant::now() >= reap_deadline {
                        return Err(HarnessError::Transport(format!(
                            "tui child is still alive after SIGKILL in `{context}`"
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(err) => {
                    if process_is_gone(&err) {
                        return Ok(());
                    }
                    return Err(HarnessError::Transport(format!(
                        "check tui child status after SIGKILL in `{context}`: {err}"
                    )));
                }
            }
        }
    }

    fn ensure_pty_available(&self) -> Result<(), HarnessError> {
        let system = native_pty_system();
        let _pair = system
            .openpty(self.pty_size)
            .map_err(|e| HarnessError::Transport(format!("open portable pty: {e}")))?;
        Ok(())
    }

    fn spawn_session(&self) -> Result<Session, HarnessError> {
        let mut command = Command::new(&self.binary);
        command.env("DATABASE_URL", &self.database_url);
        command.env("TERM", "xterm-256color");
        command.env("RUST_LOG", "error");

        let mut session = Session::spawn(command)
            .map_err(|e| HarnessError::Transport(format!("spawn tanren-tui: {e}")))?;
        session.set_expect_timeout(Some(self.expect_timeout));
        Ok(session)
    }

    fn move_to_choice(
        session: &mut Session,
        selected_index: usize,
        choice: TuiMenuChoice,
    ) -> Result<(), HarnessError> {
        let target = choice as usize;
        let delta = (target + MENU_ITEM_COUNT - selected_index) % MENU_ITEM_COUNT;
        for _ in 0..delta {
            session
                .send("\x1b[B")
                .map_err(|e| HarnessError::Transport(format!("send arrow-down: {e}")))?;
        }
        Ok(())
    }

    fn submit_form_in_session(
        &self,
        session: &mut Session,
        choice: TuiMenuChoice,
        form_title: &str,
        values: &[String],
        selected_index: &mut usize,
        wait_for_menu: bool,
        return_to_menu_after_submit: bool,
    ) -> Result<String, HarnessError> {
        if wait_for_menu {
            std::thread::sleep(Duration::from_millis(200));
        }
        Self::move_to_choice(session, *selected_index, choice)?;
        *selected_index = choice as usize;
        session
            .send("\r")
            .map_err(|e| HarnessError::Transport(format!("open menu choice: {e}")))?;
        self.wait_for_fragment(session, FORM_HINT)
            .map_err(|e| HarnessError::Transport(format!("wait form `{form_title}`: {e}")))?;

        for (idx, value) in values.iter().enumerate() {
            if !value.is_empty() {
                session
                    .send(value.as_str())
                    .map_err(|e| HarnessError::Transport(format!("type form value: {e}")))?;
            }
            if idx + 1 < values.len() {
                session
                    .send("\t")
                    .map_err(|e| HarnessError::Transport(format!("move form focus: {e}")))?;
            }
        }

        session
            .send("\r")
            .map_err(|e| HarnessError::Transport(format!("submit form: {e}")))?;

        std::thread::sleep(self.settle_timeout);
        let text = self.collect_until_quiet(session)?;
        if return_to_menu_after_submit {
            session.send("\r").map_err(|e| {
                HarnessError::Transport(format!("return to menu from outcome: {e}"))
            })?;
            self.wait_for_any_fragment(session, &[MENU_PROMPT, "Enter confirm"])
                .map_err(|e| HarnessError::Transport(format!("wait menu after outcome: {e}")))?;
            *selected_index = 0;
        }
        Ok(text)
    }

    fn collect_until_quiet(&self, session: &mut Session) -> Result<String, HarnessError> {
        let deadline = Instant::now() + self.expect_timeout;
        let mut combined = String::new();
        let mut scratch = [0_u8; 2048];
        let mut saw_outcome = false;
        let mut quiet_ticks = 0_u8;

        loop {
            if Instant::now() > deadline {
                break;
            }

            match session.try_read(&mut scratch) {
                Ok(0) => {
                    std::thread::sleep(Duration::from_millis(QUIET_TICK_MILLIS));
                    quiet_ticks = quiet_ticks.saturating_add(1);
                    if saw_outcome && quiet_ticks >= QUIET_TICKS_AFTER_OUTCOME {
                        break;
                    }
                    if !saw_outcome
                        && !combined.is_empty()
                        && quiet_ticks >= QUIET_TICKS_WITHOUT_OUTCOME
                    {
                        break;
                    }
                    if combined.is_empty() && quiet_ticks >= QUIET_TICKS_EMPTY {
                        break;
                    }
                }
                Ok(read) => {
                    let chunk = String::from_utf8_lossy(&scratch[..read]);
                    combined.push_str(&chunk);
                    quiet_ticks = 0;
                    if contains_terminal_outcome(&combined) {
                        saw_outcome = true;
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(QUIET_TICK_MILLIS));
                    quiet_ticks = quiet_ticks.saturating_add(1);
                    if saw_outcome && quiet_ticks >= QUIET_TICKS_AFTER_OUTCOME {
                        break;
                    }
                    if !saw_outcome
                        && !combined.is_empty()
                        && quiet_ticks >= QUIET_TICKS_WITHOUT_OUTCOME
                    {
                        break;
                    }
                    if combined.is_empty() && quiet_ticks >= QUIET_TICKS_EMPTY {
                        break;
                    }
                }
                Err(err) => {
                    return Err(HarnessError::Transport(format!(
                        "read tui transcript: {err}"
                    )));
                }
            }
        }

        Ok(combined)
    }

    fn wait_for_fragment(&self, session: &mut Session, fragment: &str) -> Result<(), HarnessError> {
        self.wait_for_any_fragment(session, &[fragment])
    }

    fn wait_for_any_fragment(
        &self,
        session: &mut Session,
        fragments: &[&str],
    ) -> Result<(), HarnessError> {
        let deadline = Instant::now() + self.expect_timeout;
        let mut combined = String::new();
        let mut scratch = [0_u8; 2048];

        loop {
            if Instant::now() > deadline {
                let cleaned = sanitize_terminal_text(&combined);
                return Err(HarnessError::Transport(format!(
                    "timeout waiting for `{}`; transcript: {cleaned}",
                    fragments.join("` or `")
                )));
            }

            match session.try_read(&mut scratch) {
                Ok(0) => std::thread::sleep(Duration::from_millis(30)),
                Ok(read) => {
                    let chunk = String::from_utf8_lossy(&scratch[..read]);
                    combined.push_str(&chunk);
                    let transcript = TuiTranscript {
                        text: sanitize_terminal_text(&combined),
                    };
                    if fragments
                        .iter()
                        .any(|fragment| transcript.contains(fragment))
                    {
                        return Ok(());
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(30));
                }
                Err(err) => {
                    return Err(HarnessError::Transport(format!(
                        "read tui transcript while waiting for `{}`: {err}",
                        fragments.join("` or `")
                    )));
                }
            }
        }
    }
}

fn contains_terminal_outcome(raw: &str) -> bool {
    let clean = sanitize_terminal_text(raw);
    [
        "Account created",
        "Signed in",
        "Invitation accepted",
        "Role created",
        "Role updated",
        "Role deleted",
        "Role applied",
        "Permission checked",
        "validation_failed",
        "duplicate_identifier",
        "invalid_credential",
        "invitation_expired",
        "invitation_not_found",
        "invitation_already_consumed",
        "not_found",
        "permission_denied",
        "role_as_principal_rejected",
        "internal_error",
        "sign in before role administration",
    ]
    .iter()
    .any(|needle| clean.contains(needle))
}

fn process_is_gone(err: &impl std::fmt::Display) -> bool {
    let message = err.to_string();
    message.contains("ECHILD") || message.contains("ESRCH")
}

fn process_has_exited(status: WaitStatus) -> bool {
    matches!(status, WaitStatus::Exited(..) | WaitStatus::Signaled(..))
}
