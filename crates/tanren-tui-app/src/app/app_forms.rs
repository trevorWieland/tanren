use chrono::Utc;
use tanren_app_services::{Handlers, SessionAuthenticationRequest, Store};
use tanren_contract::SessionView;
use tanren_identity_policy::AccountId;

use crate::FormState;

use super::{App, AuthenticatedSession, Screen};

impl App {
    pub(super) fn active_form(&self) -> Option<&FormState> {
        match &self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::UserConfigListSettings(s)
            | Screen::UserConfigSetSetting(s)
            | Screen::UserConfigRemoveSetting(s)
            | Screen::UserConfigListCredentials(s)
            | Screen::UserConfigAddCredential(s)
            | Screen::UserConfigUpdateCredential(s)
            | Screen::UserConfigRemoveCredential(s) => Some(s),
            _ => None,
        }
    }

    pub(super) fn active_form_mut(&mut self) -> Option<&mut FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::UserConfigListSettings(s)
            | Screen::UserConfigSetSetting(s)
            | Screen::UserConfigRemoveSetting(s)
            | Screen::UserConfigListCredentials(s)
            | Screen::UserConfigAddCredential(s)
            | Screen::UserConfigUpdateCredential(s)
            | Screen::UserConfigRemoveCredential(s) => Some(s),
            _ => None,
        }
    }

    pub(super) fn set_active_form_error(&mut self, message: String) {
        if let Some(state) = self.active_form_mut() {
            state.error = Some(message);
        }
    }

    pub(super) fn set_authenticated_session(
        &mut self,
        account_id: AccountId,
        session: &SessionView,
    ) {
        self.authenticated_session = Some(AuthenticatedSession {
            account_id,
            session_token: session.token.clone(),
        });
    }

    pub(super) fn resolve_authenticated_account_id(
        &self,
        store: &Store,
    ) -> Result<AccountId, String> {
        let session = self.authenticated_session.as_ref().ok_or_else(|| {
            "authentication_required: sign in, sign up, or accept an invitation first.".to_owned()
        })?;
        let authenticated = self
            .runtime
            .block_on(Handlers::new().authenticate_session(
                store,
                SessionAuthenticationRequest {
                    session_token: session.session_token.clone(),
                    now: Utc::now(),
                },
            ))
            .map_err(|_| "internal_error: Tanren encountered an internal error.".to_owned())?;
        let account_id = authenticated
            .ok_or_else(|| {
                "authentication_required: active session expired or was revoked; sign in again."
                    .to_owned()
            })?
            .authenticated_account_id;
        if account_id != session.account_id {
            return Err(
                "authentication_required: session principal changed; sign in again.".to_owned(),
            );
        }
        Ok(account_id)
    }
}
