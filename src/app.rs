use crate::document::{Document, RefreshOutcome};
use crate::input::Action;
use crate::search::{self, Direction};
use crate::view::{self, ScrollAnchor};
use crossterm::event::{KeyCode, KeyEvent};
use regex::bytes::Regex;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Auto-scroll (teleprompter) speed bounds and step, expressed as the
/// interval between one-line advances -- a *smaller* interval is faster.
const AUTO_SCROLL_MIN_INTERVAL: Duration = Duration::from_millis(40);
const AUTO_SCROLL_MAX_INTERVAL: Duration = Duration::from_millis(3000);
pub const AUTO_SCROLL_DEFAULT_INTERVAL: Duration = Duration::from_millis(500);
/// Multiplicative step so speed changes feel proportional at both the fast
/// and slow ends of the range, rather than a fixed-ms step that's a huge
/// relative jump when fast and imperceptible when slow.
const AUTO_SCROLL_STEP_FACTOR: f64 = 0.85;

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
    pub auto_scroll: bool,
    pub auto_scroll_interval: Duration,
    auto_scroll_next_tick: Instant,
    /// Recent-file entries for files other than the one currently open,
    /// carried forward from the loaded config so saving doesn't clobber
    /// their remembered positions with just this session's own file.
    other_recent_files: Vec<crate::config::RecentFile>,
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
            auto_scroll: false,
            auto_scroll_interval: AUTO_SCROLL_DEFAULT_INTERVAL,
            auto_scroll_next_tick: Instant::now(),
            other_recent_files: Vec::new(),
        }
    }

    /// Apply persisted settings loaded from the config file: search
    /// history, the last-used auto-scroll speed, and -- if this exact
    /// path was seen before -- jumping to its last-viewed line. Anything
    /// else session-specific (whether auto-scroll or follow are currently
    /// on) is left untouched.
    pub fn apply_config(&mut self, config: &crate::config::Config) {
        self.search_history = config.search_history.clone();
        // Keep the most recent entries (the tail) if the file has more
        // than the cap -- history is ordered oldest-first.
        let excess = self
            .search_history
            .len()
            .saturating_sub(crate::config::MAX_SEARCH_HISTORY);
        self.search_history.drain(..excess);
        self.auto_scroll_interval = config
            .auto_scroll_interval()
            .clamp(AUTO_SCROLL_MIN_INTERVAL, AUTO_SCROLL_MAX_INTERVAL);

        self.other_recent_files = config
            .recent_files
            .iter()
            .filter(|rf| rf.path != self.path)
            .cloned()
            .collect();
        if let Some(line) = config.line_for(&self.path) {
            self.anchor = ScrollAnchor {
                line_idx: line.min(self.document.last_line_index()),
                sub_row: 0,
            };
        }
    }

    /// Snapshot the settings worth persisting, for saving on exit.
    pub fn to_config(&self) -> crate::config::Config {
        let mut recent_files = self.other_recent_files.clone();
        recent_files.push(crate::config::RecentFile {
            path: self.path.clone(),
            line: self.anchor.line_idx,
        });
        let excess = recent_files
            .len()
            .saturating_sub(crate::config::MAX_RECENT_FILES);
        recent_files.drain(..excess);

        crate::config::Config {
            search_history: self.search_history.clone(),
            auto_scroll_interval_ms: self.auto_scroll_interval.as_millis() as u64,
            recent_files,
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
                self.anchor = view::scroll_down_lines(
                    &self.document,
                    self.anchor,
                    self.width,
                    self.text_height(),
                    1,
                );
                // While auto-scrolling, Up/Down double as speed nudges: Down
                // both advances a line and speeds the pace up, Up both backs
                // up a line and eases it back down -- an explicit request,
                // since a plain forward/back nudge alone felt disconnected
                // from the running pace.
                if self.auto_scroll {
                    self.adjust_auto_scroll_speed(true);
                }
            }
            Action::LineUp => {
                self.following = false;
                self.anchor = view::scroll_up_lines(&self.document, self.anchor, self.width, 1);
                if self.auto_scroll {
                    self.adjust_auto_scroll_speed(false);
                }
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
                self.anchor = view::scroll_down_lines(
                    &self.document,
                    self.anchor,
                    self.width,
                    self.text_height(),
                    n,
                )
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
            Action::ToggleAutoScroll => {
                let on = !self.auto_scroll;
                self.set_auto_scroll(on);
            }
            Action::AutoScrollFaster => self.adjust_auto_scroll_speed(true),
            Action::AutoScrollSlower => self.adjust_auto_scroll_speed(false),
        }
        self.dirty = true;
    }

    /// Turn auto-scroll on or off, e.g. from the `a` key or the
    /// `--auto-scroll` startup flag.
    pub fn set_auto_scroll(&mut self, on: bool) {
        self.auto_scroll = on;
        if on {
            self.schedule_next_auto_scroll_tick();
        }
    }

    fn adjust_auto_scroll_speed(&mut self, faster: bool) {
        let factor = if faster {
            AUTO_SCROLL_STEP_FACTOR
        } else {
            1.0 / AUTO_SCROLL_STEP_FACTOR
        };
        let new_ms = (self.auto_scroll_interval.as_millis() as f64 * factor).round() as u64;
        self.auto_scroll_interval =
            Duration::from_millis(new_ms).clamp(AUTO_SCROLL_MIN_INTERVAL, AUTO_SCROLL_MAX_INTERVAL);
        if self.auto_scroll {
            self.schedule_next_auto_scroll_tick();
        }
    }

    fn schedule_next_auto_scroll_tick(&mut self) {
        self.auto_scroll_next_tick = Instant::now() + self.auto_scroll_interval;
    }

    /// Auto-scroll only actually ticks while in Normal mode -- while a
    /// search prompt or the help overlay is open it stays logically "on"
    /// (the flag and speed are untouched), but pauses so the view doesn't
    /// silently scroll out from under whatever you're doing. It resumes
    /// from wherever you land the moment you're back to Normal mode (see
    /// `schedule_next_auto_scroll_tick` calls on returning from those
    /// modes), rather than firing a burst of catch-up ticks for the time
    /// spent paused.
    fn auto_scroll_active(&self) -> bool {
        self.auto_scroll && matches!(self.input_mode, InputMode::Normal)
    }

    /// Whether the auto-scroll timer has elapsed and it's time to advance
    /// one line. Called from the main loop each iteration.
    pub fn auto_scroll_due(&self, now: Instant) -> bool {
        self.auto_scroll_active() && now >= self.auto_scroll_next_tick
    }

    /// How long until the next auto-scroll tick, for the main loop to use
    /// as its event-poll timeout so ticks stay punctual. `None` when
    /// auto-scroll is off or paused.
    pub fn auto_scroll_wake_deadline(&self) -> Option<Instant> {
        self.auto_scroll_active()
            .then_some(self.auto_scroll_next_tick)
    }

    /// Advance one line and reschedule. Once this reaches the current end
    /// of file, it hands off to follow mode so newly appended content keeps
    /// the teleprompter moving -- auto-scroll's own timer has no way to
    /// know about future appends, but follow mode is driven by file-change
    /// events instead, so it picks up seamlessly.
    pub fn auto_scroll_tick(&mut self) {
        self.anchor = view::scroll_down_lines(
            &self.document,
            self.anchor,
            self.width,
            self.text_height(),
            1,
        );
        if view::is_at_bottom(&self.document, self.anchor, self.width, self.text_height()) {
            self.following = true;
        }
        self.schedule_next_auto_scroll_tick();
        self.dirty = true;
    }

    /// Called every iteration of the main loop to check whether the file
    /// has grown or been replaced on disk (a single cheap stat() call via
    /// Document::refresh_append when nothing changed, so polling this
    /// unconditionally rather than only on an OS file-change notification
    /// is inexpensive -- and unlike OS notifications, it can't silently
    /// fail to fire). Re-reads the appended tail (or fully reloads on
    /// rotation/truncation), and if we're following, keeps the view
    /// pinned to the new end of file.
    pub fn handle_file_changed(&mut self) {
        match self.document.refresh_append(&self.path) {
            Ok(RefreshOutcome::Unchanged) => return,
            Ok(RefreshOutcome::NeedsReload) => {
                if self.document.reload(&self.path).is_err() {
                    // Transiently unavailable (e.g. mid-rotate); try again
                    // on the next poll.
                    return;
                }
            }
            Ok(RefreshOutcome::Appended) => {}
            Err(_) => return,
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
        self.schedule_next_auto_scroll_tick();
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
                self.schedule_next_auto_scroll_tick();
            }
            KeyCode::Enter => {
                let direction = state.direction;
                let pattern = std::mem::take(&mut state.query);
                self.input_mode = InputMode::Normal;
                self.schedule_next_auto_scroll_tick();
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
                    if self.search_history.len() > crate::config::MAX_SEARCH_HISTORY {
                        self.search_history.remove(0);
                    }
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
