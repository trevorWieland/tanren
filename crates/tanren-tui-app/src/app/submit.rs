//! Submit dispatch methods for the TUI screen state machine.
//!
//! Extracted from `app.rs` to keep that file under the workspace 500-line
//! line-budget. Contains all `submit_*` methods, the `submit` dispatch,
//! and `active_form_mut`.

use std::sync::Arc;

use tanren_app_services::{AppServiceError, SourceControlProvider, Store};
use tanren_contract::ProjectFailureReason;
use tanren_provider_integrations::ProviderError;

use super::{App, FormKind, Screen};
use crate::FormState;
use crate::ui::{
    accept_invitation_outcome, active_project_none_outcome, active_project_outcome,
    connect_project_outcome, create_project_outcome, parse_accept_invitation, parse_active_project,
    parse_connect_project, parse_create_project, parse_sign_in, parse_sign_up, render_error,
    sign_in_outcome, sign_up_outcome,
};

impl App {
    fn resolve_provider(&self) -> Result<Arc<dyn SourceControlProvider>, String> {
        self.registry.resolve().map_err(|e| match e {
            ProviderError::NotConfigured => render_error(AppServiceError::Project(
                ProjectFailureReason::ProviderNotConfigured,
            )),
            _ => render_error(AppServiceError::Project(
                ProjectFailureReason::ProviderFailure,
            )),
        })
    }

    pub(super) fn submit(&mut self, kind: FormKind) {
        let Some(store) = self.store.clone() else {
            let message = self
                .store_error
                .clone()
                .unwrap_or_else(|| "store unavailable".to_owned());
            if let Some(state) = self.active_form_mut() {
                state.error = Some(message);
            }
            return;
        };
        match kind {
            FormKind::SignUp => self.submit_sign_up(&store),
            FormKind::SignIn => self.submit_sign_in(&store),
            FormKind::AcceptInvitation => self.submit_accept_invitation(&store),
            FormKind::ConnectProject => self.submit_connect_project(&store),
            FormKind::CreateProject => self.submit_create_project(&store),
            FormKind::ActiveProject => self.submit_active_project(&store),
        }
    }

    fn submit_sign_up(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::SignUp(state) = &self.screen else {
                return;
            };
            parse_sign_up(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::SignUp(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.sign_up(store.as_ref(), request));
        match result {
            Ok(response) => self.screen = Screen::Outcome(sign_up_outcome(&response)),
            Err(reason) => {
                if let Screen::SignUp(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_sign_in(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::SignIn(state) = &self.screen else {
                return;
            };
            parse_sign_in(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::SignIn(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.sign_in(store.as_ref(), request));
        match result {
            Ok(response) => self.screen = Screen::Outcome(sign_in_outcome(&response)),
            Err(reason) => {
                if let Screen::SignIn(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_accept_invitation(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::AcceptInvitation(state) = &self.screen else {
                return;
            };
            parse_accept_invitation(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.accept_invitation(store.as_ref(), request));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(accept_invitation_outcome(&response));
            }
            Err(reason) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_connect_project(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::ConnectProject(state) = &self.screen else {
                return;
            };
            parse_connect_project(state)
        };
        let (account_id, request) = match parsed {
            Ok(pair) => pair,
            Err(message) => {
                if let Screen::ConnectProject(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let scm = match self.resolve_provider() {
            Ok(p) => p,
            Err(message) => {
                if let Screen::ConnectProject(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self.runtime.block_on(self.handlers.connect_project(
            store.as_ref(),
            scm.as_ref(),
            account_id,
            request,
        ));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(connect_project_outcome(&response));
            }
            Err(reason) => {
                if let Screen::ConnectProject(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_create_project(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::CreateProject(state) = &self.screen else {
                return;
            };
            parse_create_project(state)
        };
        let (account_id, request) = match parsed {
            Ok(pair) => pair,
            Err(message) => {
                if let Screen::CreateProject(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let scm = match self.resolve_provider() {
            Ok(p) => p,
            Err(message) => {
                if let Screen::CreateProject(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self.runtime.block_on(self.handlers.create_project(
            store.as_ref(),
            scm.as_ref(),
            account_id,
            request,
        ));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(create_project_outcome(&response));
            }
            Err(reason) => {
                if let Screen::CreateProject(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_active_project(&mut self, store: &Arc<Store>) {
        let parsed = {
            let Screen::ActiveProject(state) = &self.screen else {
                return;
            };
            parse_active_project(state)
        };
        let account_id = match parsed {
            Ok(aid) => aid,
            Err(message) => {
                if let Screen::ActiveProject(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.active_project(store.as_ref(), account_id));
        match result {
            Ok(Some(view)) => self.screen = Screen::Outcome(active_project_outcome(&view)),
            Ok(None) => self.screen = Screen::Outcome(active_project_none_outcome()),
            Err(reason) => {
                if let Screen::ActiveProject(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    pub(super) fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::ConnectProject(s)
            | Screen::CreateProject(s)
            | Screen::ActiveProject(s) => Some(s),
            _ => None,
        }
    }
}
