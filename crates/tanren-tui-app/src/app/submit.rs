use super::{App, FormAction, FormKind, Screen};
use crate::ui::{
    accept_invitation_outcome, apply_role_outcome, create_role_outcome, delete_role_outcome,
    edit_role_outcome, parse_accept_invitation, parse_apply_role, parse_create_role,
    parse_delete_role, parse_edit_role, parse_permission_check, parse_sign_in, parse_sign_up,
    permission_check_outcome, render_error, render_role_error, sign_in_outcome, sign_up_outcome,
};

impl App {
    pub(super) fn dispatch_form_action(&mut self, action: FormAction, kind: FormKind) {
        match action {
            FormAction::Cancel => {
                self.screen = Screen::Menu { selected: 0 };
            }
            FormAction::Submit => self.submit(kind),
        }
    }

    fn submit(&mut self, kind: FormKind) {
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
            FormKind::SignUp => self.submit_sign_up(store.as_ref()),
            FormKind::SignIn => self.submit_sign_in(store.as_ref()),
            FormKind::AcceptInvitation => self.submit_accept_invitation(store.as_ref()),
            FormKind::CreateRole => self.submit_create_role(store.as_ref()),
            FormKind::EditRole => self.submit_edit_role(store.as_ref()),
            FormKind::DeleteRole => self.submit_delete_role(store.as_ref()),
            FormKind::ApplyRole => self.submit_apply_role(store.as_ref()),
            FormKind::CheckPermission => self.submit_check_permission(store.as_ref()),
        }
    }

    fn submit_sign_up(&mut self, store: &tanren_app_services::Store) {
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
        let result = self.runtime.block_on(self.handlers.sign_up(store, request));
        match result {
            Ok(response) => {
                self.authenticated_actor = Some(tanren_contract::RoleActor {
                    account_id: response.account.id,
                });
                self.screen = Screen::Outcome(sign_up_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SignUp(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_sign_in(&mut self, store: &tanren_app_services::Store) {
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
        let result = self.runtime.block_on(self.handlers.sign_in(store, request));
        match result {
            Ok(response) => {
                self.authenticated_actor = Some(tanren_contract::RoleActor {
                    account_id: response.account.id,
                });
                self.screen = Screen::Outcome(sign_in_outcome(&response));
            }
            Err(reason) => {
                if let Screen::SignIn(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_accept_invitation(&mut self, store: &tanren_app_services::Store) {
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
            .block_on(self.handlers.accept_invitation(store, request));
        match result {
            Ok(response) => {
                self.authenticated_actor = Some(tanren_contract::RoleActor {
                    account_id: response.account.id,
                });
                self.screen = Screen::Outcome(accept_invitation_outcome(&response));
            }
            Err(reason) => {
                if let Screen::AcceptInvitation(state) = &mut self.screen {
                    state.error = Some(render_error(reason));
                }
            }
        }
    }

    fn submit_create_role(&mut self, store: &tanren_app_services::Store) {
        let Some(actor) = self.role_actor() else {
            if let Screen::CreateRole(state) = &mut self.screen {
                state.error = Some("sign in before role administration".to_owned());
            }
            return;
        };
        let parsed = {
            let Screen::CreateRole(state) = &self.screen else {
                return;
            };
            parse_create_role(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::CreateRole(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.create_role(store, actor, request));
        match result {
            Ok(response) => self.screen = Screen::Outcome(create_role_outcome(&response)),
            Err(reason) => {
                if let Screen::CreateRole(state) = &mut self.screen {
                    state.error = Some(render_role_error(reason));
                }
            }
        }
    }

    fn submit_edit_role(&mut self, store: &tanren_app_services::Store) {
        let Some(actor) = self.role_actor() else {
            if let Screen::EditRole(state) = &mut self.screen {
                state.error = Some("sign in before role administration".to_owned());
            }
            return;
        };
        let parsed = {
            let Screen::EditRole(state) = &self.screen else {
                return;
            };
            parse_edit_role(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::EditRole(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.edit_role(store, actor, request));
        match result {
            Ok(response) => self.screen = Screen::Outcome(edit_role_outcome(&response)),
            Err(reason) => {
                if let Screen::EditRole(state) = &mut self.screen {
                    state.error = Some(render_role_error(reason));
                }
            }
        }
    }

    fn submit_delete_role(&mut self, store: &tanren_app_services::Store) {
        let Some(actor) = self.role_actor() else {
            if let Screen::DeleteRole(state) = &mut self.screen {
                state.error = Some("sign in before role administration".to_owned());
            }
            return;
        };
        let parsed = {
            let Screen::DeleteRole(state) = &self.screen else {
                return;
            };
            parse_delete_role(state)
        };
        let role = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::DeleteRole(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self.runtime.block_on(self.handlers.delete_role(
            store,
            actor,
            tanren_contract::DeleteRoleRequest { role },
        ));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(delete_role_outcome(response.role));
            }
            Err(reason) => {
                if let Screen::DeleteRole(state) = &mut self.screen {
                    state.error = Some(render_role_error(reason));
                }
            }
        }
    }

    fn submit_apply_role(&mut self, store: &tanren_app_services::Store) {
        let Some(actor) = self.role_actor() else {
            if let Screen::ApplyRole(state) = &mut self.screen {
                state.error = Some("sign in before role administration".to_owned());
            }
            return;
        };
        let parsed = {
            let Screen::ApplyRole(state) = &self.screen else {
                return;
            };
            parse_apply_role(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::ApplyRole(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.apply_role(store, actor, request));
        match result {
            Ok(response) => self.screen = Screen::Outcome(apply_role_outcome(&response)),
            Err(reason) => {
                if let Screen::ApplyRole(state) = &mut self.screen {
                    state.error = Some(render_role_error(reason));
                }
            }
        }
    }

    fn submit_check_permission(&mut self, store: &tanren_app_services::Store) {
        let Some(actor) = self.role_actor() else {
            if let Screen::CheckPermission(state) = &mut self.screen {
                state.error = Some("sign in before role administration".to_owned());
            }
            return;
        };
        let parsed = {
            let Screen::CheckPermission(state) = &self.screen else {
                return;
            };
            parse_permission_check(state)
        };
        let request = match parsed {
            Ok(req) => req,
            Err(message) => {
                if let Screen::CheckPermission(state) = &mut self.screen {
                    state.error = Some(message);
                }
                return;
            }
        };
        let result = self
            .runtime
            .block_on(self.handlers.check_permission(store, actor, request));
        match result {
            Ok(response) => {
                self.screen = Screen::Outcome(permission_check_outcome(&response));
            }
            Err(reason) => {
                if let Screen::CheckPermission(state) = &mut self.screen {
                    state.error = Some(render_role_error(reason));
                }
            }
        }
    }

    fn active_form_mut(&mut self) -> Option<&mut crate::FormState> {
        match &mut self.screen {
            Screen::SignUp(s)
            | Screen::SignIn(s)
            | Screen::AcceptInvitation(s)
            | Screen::CreateRole(s)
            | Screen::EditRole(s)
            | Screen::DeleteRole(s)
            | Screen::ApplyRole(s)
            | Screen::CheckPermission(s) => Some(s),
            _ => None,
        }
    }

    fn role_actor(&self) -> Option<tanren_contract::RoleActor> {
        self.authenticated_actor
    }
}
