use crossterm::event::{KeyCode, KeyEvent};
use tanren_contract::SignedInAccountView;

use crate::{FormState, MenuChoice};

use super::Screen;

#[derive(Debug, Clone, Copy)]
pub(super) enum FormKind {
    SignUp,
    SignIn,
    AcceptInvitation,
    SwitchActive,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FormAction {
    Submit,
    Cancel,
}

#[derive(Debug)]
pub(super) enum Effect {
    None,
    Exit,
    ReplaceScreen(Screen),
    Menu(MenuChoice),
    Form(FormAction, FormKind),
}

pub(super) fn handle_menu_key(
    selected: &mut usize,
    key: KeyEvent,
    next: &mut Option<MenuChoice>,
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
            *next = Some(MenuChoice::ALL[*selected]);
        }
        _ => {}
    }
    false
}

pub(super) fn handle_switch_active_key(
    accounts: &[SignedInAccountView],
    selected: &mut usize,
    error: &mut Option<String>,
    key: KeyEvent,
) -> Effect {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
            Effect::ReplaceScreen(Screen::Menu { selected: 0 })
        }
        KeyCode::Up => {
            if accounts.is_empty() {
                return Effect::None;
            }
            if *selected == 0 {
                *selected = accounts.len() - 1;
            } else {
                *selected -= 1;
            }
            *error = None;
            Effect::None
        }
        KeyCode::Down | KeyCode::Tab => {
            if accounts.is_empty() {
                return Effect::None;
            }
            *selected = (*selected + 1) % accounts.len();
            *error = None;
            Effect::None
        }
        KeyCode::Enter => Effect::Form(FormAction::Submit, FormKind::SwitchActive),
        _ => Effect::None,
    }
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
