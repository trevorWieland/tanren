//! Tanren terminal UI — runtime library.
//!
//! R-0001 (sub-8) promotes the runtime out of `bin/tanren-tui/src/main.rs`
//! per the thin-binary-crate profile. The binary shrinks to a wiring shell
//! that initializes tracing and calls [`run`]; everything below — terminal
//! setup/teardown, the screen state machine, form state, ratatui rendering
//! — lives here so the BDD harness can exercise the same code paths
//! through `expectrl`/`portable-pty` against the binary entry point or
//! against this library directly.
//!
//! Modules (all crate-private):
//!
//! - `app` hosts the screen state machine, the `App` struct, and the
//!   submit dispatch.
//! - `draw` hosts the ratatui rendering primitives.
//! - `notifications` hosts notification-preference form factories,
//!   outcome adapters, and validation.
//! - `ui` hosts form-field factories, outcome adapters, and validation.
//!
//! The TUI returns bearer-mode `SessionView` responses from
//! `tanren-app-services` (no cookie jar to use).

mod app;
mod draw;
mod notifications;
mod ui;

use std::io::{Stdout, stdout};

use anyhow::{Context, Result};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// Configuration for the TUI runtime. R-0001 sub-8 keeps it deliberately
/// empty — the TUI reads `DATABASE_URL` at startup so this struct exists
/// only to satisfy the `bin/tanren-tui/src/main.rs` → `run(config)`
/// contract documented in the thin-binary-crate profile.
#[derive(Debug, Default)]
pub struct Config;

impl Config {
    /// Construct the default config; bind address, allowed hosts, and
    /// API key continue to come from environment variables.
    #[must_use]
    pub const fn from_env() -> Self {
        Self
    }
}

/// Run the TUI to completion. Returns once the user exits or a setup
/// error occurs.
///
/// # Errors
///
/// Surfaces errors from terminal setup/teardown and from the runtime
/// loop. Exit-via-`q`/`Ctrl-C` is `Ok(())`.
pub fn run(_config: Config) -> Result<()> {
    let mut terminal = setup_terminal().context("setup terminal")?;
    let app_result = app::App::new().and_then(|mut app| app.run(&mut terminal));
    let teardown_result = teardown_terminal(&mut terminal).context("teardown terminal");
    app_result.and(teardown_result)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode().context("enable raw mode")?;
    let mut out = stdout();
    if let Err(err) = execute!(out, EnterAlternateScreen).context("enter alternate screen") {
        let _ = disable_raw_mode();
        return Err(err);
    }
    let backend = CrosstermBackend::new(out);
    match Terminal::new(backend).context("construct terminal") {
        Ok(terminal) => Ok(terminal),
        Err(err) => {
            let _ = execute!(stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            Err(err)
        }
    }
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode().context("disable raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).context("leave alternate screen")?;
    terminal.show_cursor().context("show cursor")?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuChoice {
    SignUp,
    SignIn,
    AcceptInvitation,
    NotificationSetPreference,
    NotificationOrgOverride,
}

impl MenuChoice {
    pub(crate) const ALL: [Self; 5] = [
        Self::SignUp,
        Self::SignIn,
        Self::AcceptInvitation,
        Self::NotificationSetPreference,
        Self::NotificationOrgOverride,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SignUp => "Sign up",
            Self::SignIn => "Sign in",
            Self::AcceptInvitation => "Accept invitation",
            Self::NotificationSetPreference => "Set notification preference",
            Self::NotificationOrgOverride => "Set org notification override",
        }
    }
}

#[derive(Debug)]
pub(crate) struct FormState {
    pub(crate) fields: Vec<FormField>,
    pub(crate) focus: usize,
    pub(crate) error: Option<String>,
}

#[derive(Debug)]
pub(crate) struct FormField {
    pub(crate) label: &'static str,
    pub(crate) secret: bool,
    pub(crate) value: String,
}

impl FormState {
    pub(crate) fn new(fields: Vec<FormField>) -> Self {
        Self {
            fields,
            focus: 0,
            error: None,
        }
    }

    pub(crate) fn cycle_focus(&mut self, forward: bool) {
        if self.fields.is_empty() {
            return;
        }
        let len = self.fields.len();
        self.focus = if forward {
            (self.focus + 1) % len
        } else {
            (self.focus + len - 1) % len
        };
    }

    pub(crate) fn push_char(&mut self, c: char) {
        if let Some(field) = self.fields.get_mut(self.focus) {
            field.value.push(c);
        }
    }

    pub(crate) fn pop_char(&mut self) {
        if let Some(field) = self.fields.get_mut(self.focus) {
            field.value.pop();
        }
    }

    pub(crate) fn value(&self, idx: usize) -> &str {
        self.fields.get(idx).map_or("", |f| f.value.as_str())
    }
}

pub(crate) use app::OutcomeView;
