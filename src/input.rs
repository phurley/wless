use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    LineDown,
    LineUp,
    PageDown,
    PageUp,
    GotoTop,
    GotoBottom,
    Redraw,
}

/// Map a key event to an `Action`, decoupled from `AppState` so the keymap
/// itself is testable in isolation.
pub fn map_key(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => Some(Action::Quit),
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => Some(Action::LineUp),
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) | (KeyCode::Enter, _) => {
            Some(Action::LineDown)
        }
        (KeyCode::Char(' '), _) | (KeyCode::Char('f'), _) | (KeyCode::PageDown, _) => {
            Some(Action::PageDown)
        }
        (KeyCode::Char('b'), _) | (KeyCode::PageUp, _) => Some(Action::PageUp),
        (KeyCode::Char('g'), _) | (KeyCode::Home, _) => Some(Action::GotoTop),
        (KeyCode::Char('G'), _) | (KeyCode::End, _) => Some(Action::GotoBottom),
        (KeyCode::Char('l'), KeyModifiers::CONTROL) | (KeyCode::Char('r'), _) => {
            Some(Action::Redraw)
        }
        _ => None,
    }
}
