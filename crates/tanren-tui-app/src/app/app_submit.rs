use chrono::Utc;
use secrecy::SecretString;
use tanren_app_services::{AccountStore, Handlers, Store};
use tanren_configuration_secrets::OwnerScope;
use tanren_contract::{
    CreateUserCredentialRequest, SessionView, UpdateUserCredentialRequest, UpsertUserSettingRequest,
};
use tanren_identity_policy::AccountId;
use tanren_store::SessionAuthenticationLookup;

use crate::FormState;
use crate::ui::{
    accept_invitation_outcome, credential_item_outcome, credential_list_outcome,
    parse_accept_invitation, parse_account_id_field, parse_credential_kind_field,
    parse_item_id_field, parse_setting_key_field, parse_setting_value_field, parse_sign_in,
    parse_sign_up, render_error, setting_item_outcome, settings_list_outcome, sign_in_outcome,
    sign_up_outcome,
};

use super::{App, AuthenticatedSession, FormKind, Screen};

impl App {
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
        let handlers = self.handlers.clone();
        match kind {
            FormKind::SignUp => self.submit_sign_up(store.as_ref(), &handlers),
            FormKind::SignIn => self.submit_sign_in(store.as_ref(), &handlers),
            FormKind::AcceptInvitation => self.submit_accept_invitation(store.as_ref(), &handlers),
            FormKind::UserConfigListSettings => {
                self.submit_user_config_list_settings(store.as_ref(), &handlers);
            }
            FormKind::UserConfigSetSetting => {
                self.submit_user_config_set_setting(store.as_ref(), &handlers);
            }
            FormKind::UserConfigRemoveSetting => {
                self.submit_user_config_remove_setting(store.as_ref(), &handlers);
            }
            FormKind::UserConfigListCredentials => {
                self.submit_user_config_list_credentials(store.as_ref(), &handlers);
            }
            FormKind::UserConfigAddCredential => {
                self.submit_user_config_add_credential(store.as_ref(), &handlers);
            }
            FormKind::UserConfigUpdateCredential => {
                self.submit_user_config_update_credential(store.as_ref(), &handlers);
            }
            FormKind::UserConfigRemoveCredential => {
                self.submit_user_config_remove_credential(store.as_ref(), &handlers);
            }
        }
    }

    fn submit_sign_up(&mut self, store: &Store, handlers: &Handlers) {
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
        let result = self.runtime.block_on(handlers.sign_up(store, request));
        match result {
            Ok(response) => {
                self.set_authenticated_session(response.account.id, &response.session);
                self.screen = Screen::Outcome(sign_up_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SignUp(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_sign_in(&mut self, store: &Store, handlers: &Handlers) {
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
        let result = self.runtime.block_on(handlers.sign_in(store, request));
        match result {
            Ok(response) => {
                self.set_authenticated_session(response.account.id, &response.session);
                self.screen = Screen::Outcome(sign_in_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SignIn(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_accept_invitation(&mut self, store: &Store, handlers: &Handlers) {
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
            .block_on(handlers.accept_invitation(store, request));
        match result {
            Ok(response) => {
                self.set_authenticated_session(response.account.id, &response.session);
                self.screen = Screen::Outcome(accept_invitation_outcome(&response));
            }
            Err(reason) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_user_config_list_settings(&mut self, store: &Store, handlers: &Handlers) {
        let requested_account_id = match self.active_form() {
            Some(state) => match parse_account_id_field(state) {
                Ok(value) => value,
                Err(message) => return self.set_active_form_error(message),
            },
            None => return,
        };
        let authenticated_account_id = match self.resolve_authenticated_account_id(store) {
            Ok(value) => value,
            Err(message) => return self.set_active_form_error(message),
        };
        let result = self.runtime.block_on(handlers.list_user_settings(
            store,
            authenticated_account_id,
            requested_account_id,
        ));
        match result {
            Ok(response) => self.screen = Screen::Outcome(settings_list_outcome(&response.items)),
            Err(reason) => {
                if let Screen::UserConfigListSettings(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_user_config_set_setting(&mut self, store: &Store, handlers: &Handlers) {
        let (requested_account_id, key, value) = match self.active_form() {
            Some(state) => {
                let requested_account_id = match parse_account_id_field(state) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                let key = match parse_setting_key_field(state, 1) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                let value = match parse_setting_value_field(key, state, 2) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                (requested_account_id, key, value)
            }
            None => return,
        };
        let authenticated_account_id = match self.resolve_authenticated_account_id(store) {
            Ok(value) => value,
            Err(message) => return self.set_active_form_error(message),
        };
        let request = UpsertUserSettingRequest { key, value };
        let result = self.runtime.block_on(handlers.upsert_user_setting(
            store,
            authenticated_account_id,
            requested_account_id,
            request,
        ));
        match result {
            Ok(response) => {
                self.screen =
                    Screen::Outcome(setting_item_outcome("Setting updated", &response.setting));
            }
            Err(reason) => {
                if let Screen::UserConfigSetSetting(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_user_config_remove_setting(&mut self, store: &Store, handlers: &Handlers) {
        let (requested_account_id, key) = match self.active_form() {
            Some(state) => {
                let requested_account_id = match parse_account_id_field(state) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                let key = match parse_setting_key_field(state, 1) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                (requested_account_id, key)
            }
            None => return,
        };
        let authenticated_account_id = match self.resolve_authenticated_account_id(store) {
            Ok(value) => value,
            Err(message) => return self.set_active_form_error(message),
        };
        let result = self.runtime.block_on(handlers.remove_user_setting(
            store,
            authenticated_account_id,
            requested_account_id,
            key,
        ));
        match result {
            Ok(response) => {
                self.screen =
                    Screen::Outcome(setting_item_outcome("Setting removed", &response.setting));
            }
            Err(reason) => {
                if let Screen::UserConfigRemoveSetting(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_user_config_list_credentials(&mut self, store: &Store, handlers: &Handlers) {
        let requested_account_id = match self.active_form() {
            Some(state) => match parse_account_id_field(state) {
                Ok(value) => value,
                Err(message) => return self.set_active_form_error(message),
            },
            None => return,
        };
        let authenticated_account_id = match self.resolve_authenticated_account_id(store) {
            Ok(value) => value,
            Err(message) => return self.set_active_form_error(message),
        };
        let result = self.runtime.block_on(handlers.list_user_credentials(
            store,
            authenticated_account_id,
            OwnerScope::User {
                account_id: requested_account_id,
            },
        ));
        match result {
            Ok(response) => self.screen = Screen::Outcome(credential_list_outcome(&response.items)),
            Err(reason) => {
                if let Screen::UserConfigListCredentials(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_user_config_add_credential(&mut self, store: &Store, handlers: &Handlers) {
        let (requested_account_id, kind, secret) = match &mut self.screen {
            Screen::UserConfigAddCredential(state) => {
                let requested_account_id = match parse_account_id_field(state) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                let kind = match parse_credential_kind_field(state, 1) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                let secret = state.value(2).to_owned();
                state.fields[2].value.clear();
                (requested_account_id, kind, secret)
            }
            _ => return,
        };
        let authenticated_account_id = match self.resolve_authenticated_account_id(store) {
            Ok(value) => value,
            Err(message) => return self.set_active_form_error(message),
        };
        let request = CreateUserCredentialRequest {
            kind,
            owner_scope: OwnerScope::User {
                account_id: requested_account_id,
            },
            value: SecretString::from(secret),
        };
        let result = self.runtime.block_on(handlers.add_user_credential(
            store,
            authenticated_account_id,
            request,
        ));
        match result {
            Ok(response) => {
                self.screen =
                    Screen::Outcome(credential_item_outcome("Credential added", &response.item));
            }
            Err(reason) => {
                if let Screen::UserConfigAddCredential(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_user_config_update_credential(&mut self, store: &Store, handlers: &Handlers) {
        let (requested_account_id, item_id, secret) = match &mut self.screen {
            Screen::UserConfigUpdateCredential(state) => {
                let requested_account_id = match parse_account_id_field(state) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                let item_id = match parse_item_id_field(state, 1) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                let secret = state.value(2).to_owned();
                state.fields[2].value.clear();
                (requested_account_id, item_id, secret)
            }
            _ => return,
        };
        let authenticated_account_id = match self.resolve_authenticated_account_id(store) {
            Ok(value) => value,
            Err(message) => return self.set_active_form_error(message),
        };
        let request = UpdateUserCredentialRequest {
            value: SecretString::from(secret),
        };
        let result = self.runtime.block_on(handlers.update_user_credential(
            store,
            authenticated_account_id,
            &item_id,
            OwnerScope::User {
                account_id: requested_account_id,
            },
            request,
        ));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(credential_item_outcome(
                    "Credential updated",
                    &response.item,
                ));
            }
            Err(reason) => {
                if let Screen::UserConfigUpdateCredential(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_user_config_remove_credential(&mut self, store: &Store, handlers: &Handlers) {
        let (requested_account_id, item_id) = match self.active_form() {
            Some(state) => {
                let requested_account_id = match parse_account_id_field(state) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                let item_id = match parse_item_id_field(state, 1) {
                    Ok(value) => value,
                    Err(message) => return self.set_active_form_error(message),
                };
                (requested_account_id, item_id)
            }
            None => return,
        };
        let authenticated_account_id = match self.resolve_authenticated_account_id(store) {
            Ok(value) => value,
            Err(message) => return self.set_active_form_error(message),
        };
        let result = self.runtime.block_on(handlers.remove_user_credential(
            store,
            authenticated_account_id,
            &item_id,
            OwnerScope::User {
                account_id: requested_account_id,
            },
        ));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(credential_item_outcome(
                    "Credential removed",
                    &response.item,
                ));
            }
            Err(reason) => {
                if let Screen::UserConfigRemoveCredential(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

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

    fn set_authenticated_session(&mut self, account_id: AccountId, session: &SessionView) {
        self.authenticated_session = Some(AuthenticatedSession {
            account_id,
            session_token: session.token.clone(),
        });
    }

    fn resolve_authenticated_account_id(&self, store: &Store) -> Result<AccountId, String> {
        let session = self.authenticated_session.as_ref().ok_or_else(|| {
            "authentication_required: sign in, sign up, or accept an invitation first.".to_owned()
        })?;
        let authenticated = self
            .runtime
            .block_on(store.authenticate_session(SessionAuthenticationLookup {
                session_token: session.session_token.clone(),
                now: Utc::now(),
            }))
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
