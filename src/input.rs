use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    LineDown,
    LineUp,
    PageDown,
    PageUp,
    HalfPageDown,
    HalfPageUp,
    GotoTop,
    GotoBottom,
    Redraw,
    SearchForward,
    SearchBackward,
    RepeatSearchSame,
    RepeatSearchOpposite,
    ClearSearch,
    ToggleHelp,
    Follow,
    ToggleAutoScroll,
    AutoScrollFaster,
    AutoScrollSlower,
}

/// Map a key event to an `Action`, decoupled from `AppState` so the keymap
/// itself is testable in isolation. Only used in normal mode -- search text
/// entry is handled separately since it needs raw characters.
pub fn map_key(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) => Some(Action::Quit),
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => Some(Action::LineUp),
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) | (KeyCode::Enter, _) => {
            Some(Action::LineDown)
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => Some(Action::HalfPageDown),
        (KeyCode::Char('u'), KeyModifiers::CONTROL) => Some(Action::HalfPageUp),
        (KeyCode::Char(' '), _) | (KeyCode::Char('f'), _) | (KeyCode::PageDown, _) => {
            Some(Action::PageDown)
        }
        (KeyCode::Char('b'), _) | (KeyCode::PageUp, _) => Some(Action::PageUp),
        (KeyCode::Char('g'), _) | (KeyCode::Home, _) => Some(Action::GotoTop),
        (KeyCode::Char('G'), _) | (KeyCode::End, _) => Some(Action::GotoBottom),
        (KeyCode::Char('l'), KeyModifiers::CONTROL) | (KeyCode::Char('r'), _) => {
            Some(Action::Redraw)
        }
        (KeyCode::Char('/'), _) => Some(Action::SearchForward),
        (KeyCode::Char('?'), _) => Some(Action::SearchBackward),
        (KeyCode::Char('n'), _) => Some(Action::RepeatSearchSame),
        (KeyCode::Char('N'), _) => Some(Action::RepeatSearchOpposite),
        (KeyCode::Esc, _) => Some(Action::ClearSearch),
        (KeyCode::Char('h'), _) | (KeyCode::Char('H'), _) => Some(Action::ToggleHelp),
        (KeyCode::Char('F'), _) => Some(Action::Follow),
        (KeyCode::Char('a'), _) => Some(Action::ToggleAutoScroll),
        (KeyCode::Char('+'), _) | (KeyCode::Char('='), _) => Some(Action::AutoScrollFaster),
        (KeyCode::Char('-'), _) => Some(Action::AutoScrollSlower),
        _ => None,
    }
}
