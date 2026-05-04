//! Narrowly-scoped public harness module for BDD driving of the TUI.
//!
//! Exposes just enough surface to construct a [`TuiDriver`] without a
//! real terminal, push [`KeyEvent`]s, and observe the resulting screen
//! discriminant and banner text. No internals beyond what the harness
//! consumes are leaked.

use std::sync::Arc;

use crossterm::event::KeyEvent;
use tanren_app_services::Store;
use tanren_identity_policy::SessionToken;

/// Discriminant of the current TUI screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenKind {
    Menu,
    SignUp,
    SignIn,
    AcceptInvitation,
    Outcome,
    Dashboard,
    OrgCreate,
    OrgList,
}

/// Read-only snapshot of the current screen state visible to the
/// harness.
#[derive(Debug, Clone)]
pub struct ScreenSnapshot {
    /// Which screen is currently active.
    pub kind: ScreenKind,
    /// Error or status banner text, if any. Populated when a form
    /// submission fails — the string contains a `code:` token the
    /// harness can assert on.
    pub banner: Option<String>,
}

/// Drives the TUI `App` without a real terminal.
/// Wraps the internal state machine and exposes only keypress injection and
/// screen observation.
pub struct TuiDriver {
    app: crate::app::App,
}

impl std::fmt::Debug for TuiDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TuiDriver")
            .field("screen", &self.app.screen_kind())
            .finish_non_exhaustive()
    }
}

impl TuiDriver {
    /// Construct a driver backed by a fresh tokio runtime and a store
    /// connection from `DATABASE_URL`.
    ///
    /// # Errors
    ///
    /// Returns an error if the tokio runtime or store connection fails.
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            app: crate::app::App::new()?,
        })
    }

    /// Construct a driver backed by a pre-existing store (shared with
    /// the rest of the BDD harness).
    ///
    /// # Errors
    ///
    /// Returns an error if the tokio runtime cannot be created.
    pub fn with_store(store: Arc<Store>) -> anyhow::Result<Self> {
        Ok(Self {
            app: crate::app::App::with_store(store)?,
        })
    }

    /// Push a key event into the state machine. Returns `true` when
    /// the app signals exit (Ctrl-C / q on menu).
    pub fn push_key(&mut self, key: KeyEvent) -> bool {
        self.app.handle_key(key)
    }

    /// Observe the current screen discriminant and any banner text.
    pub fn screen(&self) -> ScreenSnapshot {
        ScreenSnapshot {
            kind: self.app.screen_kind(),
            banner: self.app.screen_banner(),
        }
    }

    /// Inject a session token (e.g. obtained from a prior in-process
    /// sign-up) so org-scoped screens can function.
    pub fn set_session_token(&mut self, token: SessionToken) {
        self.app.set_session_token(token);
    }

    /// Read back the current session token, if any.
    pub fn session_token(&self) -> Option<SessionToken> {
        self.app.session_token_ref().cloned()
    }

    /// Jump directly to the Dashboard screen. Intended for test
    /// harness use only — the user-facing flow reaches the Dashboard
    /// through sign-in.
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn navigate_to_dashboard(&mut self) {
        self.app.navigate_to_dashboard();
    }
}
