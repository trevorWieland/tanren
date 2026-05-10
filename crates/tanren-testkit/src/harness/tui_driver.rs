use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use expectrl::Session;
use portable_pty::{PtySize, native_pty_system};

use super::HarnessError;

const MENU_PROMPT: &str = "Choose an action";

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

        let text =
            self.submit_form_in_session(&mut session, choice, form_title, values, true, false)?;

        session
            .send("qq")
            .map_err(|e| HarnessError::Transport(format!("quit tui after submit: {e}")))?;

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
        self.submit_form_in_session(
            &mut session,
            TuiMenuChoice::SignIn,
            "Sign in",
            &[email.to_owned(), password.to_owned()],
            true,
            true,
        )?;
        let text =
            self.submit_form_in_session(&mut session, choice, form_title, values, false, false)?;

        session
            .send("qq")
            .map_err(|e| HarnessError::Transport(format!("quit tui after submit: {e}")))?;

        Ok(TuiTranscript {
            text: sanitize_terminal_text(&text),
        })
    }

    pub(crate) fn probe_startup(&self) -> Result<(), HarnessError> {
        self.ensure_pty_available()?;
        let mut session = self.spawn_session()?;
        std::thread::sleep(Duration::from_millis(150));
        session
            .send("qq")
            .map_err(|e| HarnessError::Transport(format!("quit tui after startup probe: {e}")))?;
        Ok(())
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

    fn move_to_choice(session: &mut Session, choice: TuiMenuChoice) -> Result<(), HarnessError> {
        for _ in 0..choice as usize {
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
        wait_for_menu: bool,
        return_to_menu_after_submit: bool,
    ) -> Result<String, HarnessError> {
        if wait_for_menu {
            self.wait_for_any_fragment(session, &[MENU_PROMPT, "Sign up", "Sign in"])
                .map_err(|e| HarnessError::Transport(format!("wait menu prompt: {e}")))?;
        }
        Self::move_to_choice(session, choice)?;
        session
            .send("\r")
            .map_err(|e| HarnessError::Transport(format!("open menu choice: {e}")))?;
        self.wait_for_fragment(session, form_title)
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
                    std::thread::sleep(Duration::from_millis(30));
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
                    std::thread::sleep(Duration::from_millis(30));
                    if saw_outcome {
                        quiet_ticks = quiet_ticks.saturating_add(1);
                        if quiet_ticks >= 4 {
                            break;
                        }
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

pub(crate) fn locate_or_build_tui_binary() -> Result<PathBuf, HarnessError> {
    match locate_workspace_binary("tanren-tui") {
        Ok(path) => Ok(path),
        Err(initial) => {
            build_workspace_binary("tanren-tui")?;
            locate_workspace_binary("tanren-tui").map_err(|final_err| {
                HarnessError::Transport(format!(
                    "{initial}; attempted `cargo build --bin tanren-tui` but binary is still missing: {final_err}"
                ))
            })
        }
    }
}

fn locate_workspace_binary(name: &str) -> Result<PathBuf, HarnessError> {
    if let Ok(explicit) = std::env::var(format!(
        "TANREN_BIN_{}",
        name.replace('-', "_").to_uppercase()
    )) {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Ok(path);
        }
    }

    let exe = std::env::current_exe()
        .map_err(|e| HarnessError::Transport(format!("current exe: {e}")))?;
    let dir = exe
        .parent()
        .ok_or_else(|| HarnessError::Transport("current exe has no parent".to_owned()))?;

    let mut candidate = dir.join(name);
    if cfg!(windows) {
        candidate.set_extension("exe");
    }
    if candidate.exists() {
        return Ok(candidate);
    }

    let mut cursor = dir;
    while let Some(parent) = cursor.parent() {
        for profile in ["debug", "release"] {
            let mut probe = parent.join("target").join(profile).join(name);
            if cfg!(windows) {
                probe.set_extension("exe");
            }
            if probe.exists() {
                return Ok(probe);
            }
        }
        cursor = parent;
    }

    Err(HarnessError::Transport(format!(
        "binary `{name}` not found alongside test executable {} — run `cargo build --workspace`",
        exe.display()
    )))
}

fn build_workspace_binary(name: &str) -> Result<(), HarnessError> {
    let output = Command::new("cargo")
        .args(["build", "-q", "--locked", "--bin", name])
        .current_dir(workspace_root())
        .output()
        .map_err(|e| HarnessError::Transport(format!("spawn cargo build for `{name}`: {e}")))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(HarnessError::Transport(format!(
        "cargo build --bin {name} failed: {stderr}"
    )))
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root must exist")
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

fn sanitize_terminal_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if let Some('[') = chars.peek().copied() {
                let _ = chars.next();
                for code in chars.by_ref() {
                    if ('@'..='~').contains(&code) {
                        break;
                    }
                }
                continue;
            }
            continue;
        }
        if ch != '\r' {
            out.push(ch);
        }
    }

    out
}

fn normalize_for_match(raw: &str) -> String {
    raw.chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}
