use crate::document::{Document, RefreshOutcome};
use crate::input::Action;
use crate::search::{self, Direction};
use crate::view::{self, ScrollAnchor};
use crossterm::event::{KeyCode, KeyEvent};
use regex::bytes::Regex;
use std::path::PathBuf;

/// State of the search text-entry prompt (open via `/` or `?`).
pub struct SearchInput {
    pub direction: Direction,
    pub query: String,
    /// Index into `search_history` (0 = most recent) while browsing with
    /// Up/Down; `None` means the query is live-typed, not from history.
    history_pos: Option<usize>,
    /// The live-typed query, saved so Down can return to it after Up has
    /// been used to browse history.
    draft: String,
}

pub enum InputMode {
    Normal,
    Search(SearchInput),
    Help,
}

pub struct AppState {
    pub document: Document,
    pub path: PathBuf,
    pub filename: String,
    pub anchor: ScrollAnchor,
    pub width: u16,
    pub height: u16,
    pub dirty: bool,
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub last_pattern: Option<Regex>,
    pub last_direction: Direction,
    pub search_history: Vec<String>,
    pub status_message: Option<String>,
    pub following: bool,
}

impl AppState {
    pub fn new(
        document: Document,
        path: PathBuf,
        filename: String,
        width: u16,
        height: u16,
    ) -> Self {
        AppState {
            document,
            path,
            filename,
            anchor: ScrollAnchor::top(),
            width,
            height,
            dirty: true,
            should_quit: false,
            input_mode: InputMode::Normal,
            last_pattern: None,
            last_direction: Direction::Forward,
            search_history: Vec::new(),
            status_message: None,
            following: false,
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
                self.following = false;
                self.anchor = view::scroll_up_lines(&self.document, self.anchor, self.width, 1)
            }
            Action::PageDown => {
                self.anchor =
                    view::page_down(&self.document, self.anchor, self.width, self.text_height())
            }
            Action::PageUp => {
                self.following = false;
                self.anchor =
                    view::page_up(&self.document, self.anchor, self.width, self.text_height())
            }
            Action::HalfPageDown => {
                let n = (self.text_height() as usize / 2).max(1);
                self.anchor = view::scroll_down_lines(&self.document, self.anchor, self.width, n)
            }
            Action::HalfPageUp => {
                self.following = false;
                let n = (self.text_height() as usize / 2).max(1);
                self.anchor = view::scroll_up_lines(&self.document, self.anchor, self.width, n)
            }
            Action::GotoTop => {
                self.following = false;
                self.anchor = view::goto_top()
            }
            Action::GotoBottom => {
                self.anchor = view::goto_bottom(&self.document, self.width, self.text_height())
            }
            Action::Redraw => {}
            Action::SearchForward => self.open_search(Direction::Forward),
            Action::SearchBackward => self.open_search(Direction::Backward),
            Action::RepeatSearchSame => {
                self.following = false;
                let dir = self.last_direction;
                self.run_search(dir);
            }
            Action::RepeatSearchOpposite => {
                self.following = false;
                let dir = self.last_direction.opposite();
                self.run_search(dir);
            }
            Action::ClearSearch => self.last_pattern = None,
            Action::ToggleHelp => self.input_mode = InputMode::Help,
            Action::Follow => {
                self.following = true;
                self.anchor = view::goto_bottom(&self.document, self.width, self.text_height());
            }
        }
        self.dirty = true;
    }

    /// Called when the file-watcher reports the file changed on disk.
    /// Re-reads the appended tail (or fully reloads on rotation/
    /// truncation), and if we're following, keeps the view pinned to the
    /// new end of file.
    pub fn handle_file_changed(&mut self) {
        let outcome = self.document.refresh_append(&self.path);
        match outcome {
            Ok(RefreshOutcome::NeedsReload) => {
                let _ = self.document.reload(&self.path);
            }
            Ok(_) => {}
            Err(_) => {
                // The file may be transiently unavailable (e.g. mid-rotate);
                // just skip this update and try again on the next event.
                return;
            }
        }
        if self.following {
            self.anchor = view::goto_bottom(&self.document, self.width, self.text_height());
        } else {
            // Even outside follow mode, clamp so a shrink/rotation can't
            // leave the anchor pointing past the new end of file.
            self.anchor.line_idx = self.anchor.line_idx.min(self.document.last_line_index());
        }
        self.dirty = true;
    }

    fn open_search(&mut self, direction: Direction) {
        self.following = false;
        self.input_mode = InputMode::Search(SearchInput {
            direction,
            query: String::new(),
            history_pos: None,
            draft: String::new(),
        });
    }

    /// Any key press while the help overlay is showing closes it.
    pub fn close_help(&mut self) {
        self.input_mode = InputMode::Normal;
        self.dirty = true;
    }

    /// Handle a raw key while in search text-entry mode.
    pub fn handle_search_key(&mut self, key: KeyEvent) {
        let InputMode::Search(state) = &mut self.input_mode else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                let direction = state.direction;
                let pattern = std::mem::take(&mut state.query);
                self.input_mode = InputMode::Normal;
                self.submit_search(direction, &pattern);
            }
            KeyCode::Backspace => {
                state.query.pop();
                state.history_pos = None;
            }
            KeyCode::Up => self.history_older(),
            KeyCode::Down => self.history_newer(),
            KeyCode::Char(c) => {
                state.query.push(c);
                state.history_pos = None;
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn history_older(&mut self) {
        let InputMode::Search(state) = &mut self.input_mode else {
            return;
        };
        if self.search_history.is_empty() {
            return;
        }
        let next = state.history_pos.map_or(0, |i| {
            (i + 1).min(self.search_history.len().saturating_sub(1))
        });
        if state.history_pos.is_none() {
            state.draft = state.query.clone();
        }
        state.history_pos = Some(next);
        state.query = self.search_history[self.search_history.len() - 1 - next].clone();
    }

    fn history_newer(&mut self) {
        let InputMode::Search(state) = &mut self.input_mode else {
            return;
        };
        match state.history_pos {
            None => {}
            Some(0) => {
                state.history_pos = None;
                state.query = state.draft.clone();
            }
            Some(i) => {
                let next = i - 1;
                state.history_pos = Some(next);
                state.query = self.search_history[self.search_history.len() - 1 - next].clone();
            }
        }
    }

    fn submit_search(&mut self, direction: Direction, pattern: &str) {
        // An empty query (just pressing Enter) repeats the last search
        // pattern in the chosen direction, matching `less`'s behavior --
        // this also sidesteps ever compiling an empty regex, whose
        // zero-width matches can land off UTF-8 character boundaries.
        if pattern.is_empty() {
            if self.last_pattern.is_none() {
                self.status_message = Some("No previous search pattern".to_string());
                return;
            }
            self.last_direction = direction;
            self.run_search(direction);
            return;
        }

        match search::compile(pattern) {
            Ok(re) => {
                self.last_pattern = Some(re);
                self.last_direction = direction;
                if self.search_history.last().map(String::as_str) != Some(pattern) {
                    self.search_history.push(pattern.to_string());
                }
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
                let offset = self.document.floor_char_boundary(m.line, m.range.start);
                let sub_row = view::sub_row_for_offset(&self.document, m.line, offset, self.width);
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
