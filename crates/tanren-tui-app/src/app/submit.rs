use super::{App, Screen};
use crate::session::persist_session;
use crate::ui::{
    accept_invitation_outcome, parse_accept_invitation, parse_sign_in, parse_sign_up, render_error,
    sign_in_outcome, sign_up_outcome,
};

impl App {
    pub(super) fn submit_sign_up(&mut self) {
        let Some(store) = self.store.clone() else {
            let message = self
                .store_error
                .clone()
                .unwrap_or_else(|| "store unavailable".to_owned());
            if let Screen::SignUp(state) = &mut self.screen {
                state.error = Some(message);
            }
            return;
        };
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
            Ok(response) => {
                if let Err(err) =
                    persist_session(response.account.id, response.session.token.expose_secret())
                {
                    if let Screen::SignUp(state) = &mut self.screen {
                        state.error = Some(format!("internal_error: {err}"));
                    }
                    return;
                }
                self.screen = Screen::Outcome(sign_up_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SignUp(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    pub(super) fn submit_sign_in(&mut self) {
        let Some(store) = self.store.clone() else {
            let message = self
                .store_error
                .clone()
                .unwrap_or_else(|| "store unavailable".to_owned());
            if let Screen::SignIn(state) = &mut self.screen {
                state.error = Some(message);
            }
            return;
        };
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
            Ok(response) => {
                if let Err(err) =
                    persist_session(response.account.id, response.session.token.expose_secret())
                {
                    if let Screen::SignIn(state) = &mut self.screen {
                        state.error = Some(format!("internal_error: {err}"));
                    }
                    return;
                }
                self.screen = Screen::Outcome(sign_in_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SignIn(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    pub(super) fn submit_accept_invitation(&mut self) {
        let Some(store) = self.store.clone() else {
            let message = self
                .store_error
                .clone()
                .unwrap_or_else(|| "store unavailable".to_owned());
            if let Screen::AcceptInvitation(state) = &mut self.screen {
                state.error = Some(message);
            }
            return;
        };
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
                if let Err(err) =
                    persist_session(response.account.id, response.session.token.expose_secret())
                {
                    if let Screen::AcceptInvitation(state) = &mut self.screen {
                        state.error = Some(format!("internal_error: {err}"));
                    }
                    return;
                }
                self.screen = Screen::Outcome(accept_invitation_outcome(&response));
            }
            Err(reason) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }
}
