use crate::document::Document;
use crate::input::Action;
use crate::search::{self, Direction};
use crate::view::{self, ScrollAnchor};
use crossterm::event::{KeyCode, KeyEvent};
use regex::bytes::Regex;

pub enum InputMode {
    Normal,
    Search { direction: Direction, query: String },
}

pub struct AppState {
    pub document: Document,
    pub filename: String,
    pub anchor: ScrollAnchor,
    pub width: u16,
    pub height: u16,
    pub dirty: bool,
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub last_pattern: Option<Regex>,
    pub last_direction: Direction,
    pub status_message: Option<String>,
}

impl AppState {
    pub fn new(document: Document, filename: String, width: u16, height: u16) -> Self {
        AppState {
            document,
            filename,
            anchor: ScrollAnchor::top(),
            width,
            height,
            dirty: true,
            should_quit: false,
            input_mode: InputMode::Normal,
            last_pattern: None,
            last_direction: Direction::Forward,
            status_message: None,
        }
    }

    fn text_height(&self) -> u16 {
        self.height.saturating_sub(1) // one row reserved for the status bar
    }

    pub fn handle_action(&mut self, action: Action) {
        self.status_message = None;
        match action {
            Action::Quit => self.should_quit = true,
            Action::LineDown => {
                self.anchor = view::scroll_down_lines(&self.document, self.anchor, self.width, 1)
            }
            Action::LineUp => {
                self.anchor = view::scroll_up_lines(&self.document, self.anchor, self.width, 1)
            }
            Action::PageDown => {
                self.anchor =
                    view::page_down(&self.document, self.anchor, self.width, self.text_height())
            }
            Action::PageUp => {
                self.anchor =
                    view::page_up(&self.document, self.anchor, self.width, self.text_height())
            }
            Action::HalfPageDown => {
                let n = (self.text_height() as usize / 2).max(1);
                self.anchor = view::scroll_down_lines(&self.document, self.anchor, self.width, n)
            }
            Action::HalfPageUp => {
                let n = (self.text_height() as usize / 2).max(1);
                self.anchor = view::scroll_up_lines(&self.document, self.anchor, self.width, n)
            }
            Action::GotoTop => self.anchor = view::goto_top(),
            Action::GotoBottom => {
                self.anchor = view::goto_bottom(&self.document, self.width, self.text_height())
            }
            Action::Redraw => {}
            Action::SearchForward => {
                self.input_mode = InputMode::Search {
                    direction: Direction::Forward,
                    query: String::new(),
                };
            }
            Action::SearchBackward => {
                self.input_mode = InputMode::Search {
                    direction: Direction::Backward,
                    query: String::new(),
                };
            }
            Action::RepeatSearchSame => {
                let dir = self.last_direction;
                self.run_search(dir);
            }
            Action::RepeatSearchOpposite => {
                let dir = self.last_direction.opposite();
                self.run_search(dir);
            }
            Action::ClearSearch => self.last_pattern = None,
        }
        self.dirty = true;
    }

    /// Handle a raw key while in search text-entry mode.
    pub fn handle_search_key(&mut self, key: KeyEvent) {
        let InputMode::Search { direction, query } = &mut self.input_mode else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                let direction = *direction;
                let pattern = std::mem::take(query);
                self.input_mode = InputMode::Normal;
                self.submit_search(direction, &pattern);
            }
            KeyCode::Backspace => {
                query.pop();
            }
            KeyCode::Char(c) => {
                query.push(c);
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn submit_search(&mut self, direction: Direction, pattern: &str) {
        match search::compile(pattern) {
            Ok(re) => {
                self.last_pattern = Some(re);
                self.last_direction = direction;
                self.run_search(direction);
            }
            Err(err) => {
                self.status_message = Some(format!("Invalid pattern: {err}"));
            }
        }
    }

    fn run_search(&mut self, direction: Direction) {
        let Some(re) = self.last_pattern.clone() else {
            self.status_message = Some("No previous search pattern".to_string());
            return;
        };
        let from_line = self.anchor.line_idx;
        let found = match direction {
            Direction::Forward => search::search_forward(&self.document, &re, from_line),
            Direction::Backward => search::search_backward(&self.document, &re, from_line),
        };
        match found {
            Some(m) => {
                let sub_row =
                    view::sub_row_for_offset(&self.document, m.line, m.range.start, self.width);
                self.anchor = ScrollAnchor {
                    line_idx: m.line,
                    sub_row,
                };
                self.status_message = None;
            }
            None => {
                self.status_message = Some("Pattern not found".to_string());
            }
        }
    }

    pub fn handle_resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.anchor = view::reflow_anchor(&self.document, self.anchor, self.width);
        self.dirty = true;
    }
}
