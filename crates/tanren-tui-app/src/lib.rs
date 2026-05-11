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
//! - `ui` hosts form-field factories, outcome adapters, and validation.
//!
//! The TUI runs against the HTTP control-plane via a typed app-service
//! client boundary in `app::api`.

mod app;
mod ui;

use std::io::{Stdout, stdout};

use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use thiserror::Error;

/// Configuration for the TUI runtime. R-0001 sub-8 keeps it deliberately
/// empty — the TUI reads `TANREN_API_BASE_URL` at startup so this struct exists
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

#[derive(Debug, Error)]
pub enum TuiError {
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    App(String),
}

/// Run the TUI to completion. Returns once the user exits or a setup
/// error occurs.
///
/// # Errors
///
/// Surfaces errors from terminal setup/teardown and from the runtime
/// loop. Exit-via-`q`/`Ctrl-C` is `Ok(())`.
pub fn run(_config: Config) -> Result<(), TuiError> {
    let mut terminal = setup_terminal()?;
    let app_result = app::App::new()
        .map_err(|err| TuiError::App(err.to_string()))
        .and_then(|mut app| {
            app.run(&mut terminal)
                .map_err(|err| TuiError::App(err.to_string()))
        });
    let teardown_result = teardown_terminal(&mut terminal);
    app_result.and(teardown_result)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, TuiError> {
    enable_raw_mode().map_err(|source| TuiError::Io {
        context: "setup terminal: enable raw mode",
        source,
    })?;
    let mut out = stdout();
    if let Err(source) = execute!(out, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(TuiError::Io {
            context: "setup terminal: enter alternate screen",
            source,
        });
    }
    let backend = CrosstermBackend::new(out);
    match Terminal::new(backend) {
        Ok(terminal) => Ok(terminal),
        Err(source) => {
            let _ = execute!(stdout(), LeaveAlternateScreen);
            let _ = disable_raw_mode();
            Err(TuiError::Io {
                context: "setup terminal: construct terminal",
                source,
            })
        }
    }
}

fn teardown_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<(), TuiError> {
    disable_raw_mode().map_err(|source| TuiError::Io {
        context: "teardown terminal: disable raw mode",
        source,
    })?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(|source| TuiError::Io {
        context: "teardown terminal: leave alternate screen",
        source,
    })?;
    terminal.show_cursor().map_err(|source| TuiError::Io {
        context: "teardown terminal: show cursor",
        source,
    })?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuChoice {
    SignUp,
    SignIn,
    AcceptInvitation,
    CreateOrganization,
    ListOrganizations,
    CheckOrganizationPermission,
    ListOrganizationMembers,
}

impl MenuChoice {
    pub(crate) const ALL: [Self; 7] = [
        Self::SignUp,
        Self::SignIn,
        Self::AcceptInvitation,
        Self::CreateOrganization,
        Self::ListOrganizations,
        Self::CheckOrganizationPermission,
        Self::ListOrganizationMembers,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SignUp => "Sign up",
            Self::SignIn => "Sign in",
            Self::AcceptInvitation => "Accept invitation",
            Self::CreateOrganization => "Create organization",
            Self::ListOrganizations => "List organizations",
            Self::CheckOrganizationPermission => "Check org permission",
            Self::ListOrganizationMembers => "List org members",
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

// Re-export OutcomeView at crate root so `draw.rs`'s
// `use crate::OutcomeView` continues to resolve.
pub(crate) use app::OutcomeView;
