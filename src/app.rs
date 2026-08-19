use crate::document::Document;
use crate::input::Action;
use crate::view::{self, ScrollAnchor};

pub struct AppState {
    pub document: Document,
    pub filename: String,
    pub anchor: ScrollAnchor,
    pub width: u16,
    pub height: u16,
    pub dirty: bool,
    pub should_quit: bool,
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
        }
    }

    fn text_height(&self) -> u16 {
        self.height.saturating_sub(1) // one row reserved for the status bar
    }

    pub fn handle_action(&mut self, action: Action) {
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
            Action::GotoTop => self.anchor = view::goto_top(),
            Action::GotoBottom => {
                self.anchor = view::goto_bottom(&self.document, self.width, self.text_height())
            }
            Action::Redraw => {}
        }
        self.dirty = true;
    }

    pub fn handle_resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.anchor = view::reflow_anchor(&self.document, self.anchor, self.width);
        self.dirty = true;
    }
}
