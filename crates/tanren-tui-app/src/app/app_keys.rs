use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::{
    accept_invitation_fields, config_add_credential_fields, config_list_credentials_fields,
    config_list_settings_fields, config_remove_credential_fields, config_remove_setting_fields,
    config_set_setting_fields, config_update_credential_fields, sign_in_fields, sign_up_fields,
};
use crate::{FormState, MenuChoice};

use super::{FormAction, Screen};

pub(super) fn handle_menu_key(
    selected: &mut usize,
    key: KeyEvent,
    next: &mut Option<Screen>,
) -> bool {
    match key.code {
        KeyCode::Char('q' | 'Q') | KeyCode::Esc => return true,
        KeyCode::Up => {
            if *selected == 0 {
                *selected = MenuChoice::ALL.len() - 1;
            } else {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % MenuChoice::ALL.len();
        }
        KeyCode::Enter => {
            let choice = MenuChoice::ALL[*selected];
            *next = Some(match choice {
                MenuChoice::SignUp => Screen::SignUp(FormState::new(sign_up_fields())),
                MenuChoice::SignIn => Screen::SignIn(FormState::new(sign_in_fields())),
                MenuChoice::AcceptInvitation => {
                    Screen::AcceptInvitation(FormState::new(accept_invitation_fields()))
                }
                MenuChoice::UserConfiguration => Screen::UserConfigMenu { selected: 0 },
            });
        }
        _ => {}
    }
    false
}

pub(super) fn handle_user_config_menu_key(
    selected: &mut usize,
    key: KeyEvent,
    next: &mut Option<Screen>,
) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => return true,
        KeyCode::Up => {
            if *selected == 0 {
                *selected = 2;
            } else {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % 3;
        }
        KeyCode::Enter => {
            *next = Some(match *selected {
                0 => Screen::UserConfigSettingsMenu { selected: 0 },
                1 => Screen::UserConfigCredentialsMenu { selected: 0 },
                _ => Screen::Menu { selected: 0 },
            });
        }
        _ => {}
    }
    false
}

pub(super) fn handle_user_config_settings_menu_key(
    selected: &mut usize,
    key: KeyEvent,
    next: &mut Option<Screen>,
) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => return true,
        KeyCode::Up => {
            if *selected == 0 {
                *selected = 3;
            } else {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % 4;
        }
        KeyCode::Enter => {
            *next = Some(match *selected {
                0 => Screen::UserConfigListSettings(FormState::new(config_list_settings_fields())),
                1 => Screen::UserConfigSetSetting(FormState::new(config_set_setting_fields())),
                2 => {
                    Screen::UserConfigRemoveSetting(FormState::new(config_remove_setting_fields()))
                }
                _ => Screen::UserConfigMenu { selected: 0 },
            });
        }
        _ => {}
    }
    false
}

pub(super) fn handle_user_config_credentials_menu_key(
    selected: &mut usize,
    key: KeyEvent,
    next: &mut Option<Screen>,
) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => return true,
        KeyCode::Up => {
            if *selected == 0 {
                *selected = 4;
            } else {
                *selected -= 1;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            *selected = (*selected + 1) % 5;
        }
        KeyCode::Enter => {
            *next = Some(match *selected {
                0 => Screen::UserConfigListCredentials(FormState::new(
                    config_list_credentials_fields(),
                )),
                1 => {
                    Screen::UserConfigAddCredential(FormState::new(config_add_credential_fields()))
                }
                2 => Screen::UserConfigUpdateCredential(FormState::new(
                    config_update_credential_fields(),
                )),
                3 => Screen::UserConfigRemoveCredential(FormState::new(
                    config_remove_credential_fields(),
                )),
                _ => Screen::UserConfigMenu { selected: 0 },
            });
        }
        _ => {}
    }
    false
}

pub(super) fn handle_form_key(state: &mut FormState, key: KeyEvent) -> Option<FormAction> {
    match key.code {
        KeyCode::Esc => Some(FormAction::Cancel),
        KeyCode::Enter => Some(FormAction::Submit),
        KeyCode::Tab | KeyCode::Down => {
            state.cycle_focus(true);
            None
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.cycle_focus(false);
            None
        }
        KeyCode::Backspace => {
            state.pop_char();
            None
        }
        KeyCode::Char(c) => {
            state.push_char(c);
            None
        }
        _ => None,
    }
}

pub(super) fn is_press(key: &KeyEvent) -> bool {
    use crossterm::event::KeyEventKind;
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}
