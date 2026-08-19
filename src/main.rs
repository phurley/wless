mod app;
mod document;
mod input;
mod render;
mod terminal;
mod view;
mod wrap;

use app::AppState;
use clap::Parser;
use crossterm::event::{self, Event};
use document::Document;
use std::path::PathBuf;
use std::time::Duration;
use terminal::TerminalGuard;

/// wless -- a word-wrapping, auto-following pager.
#[derive(Parser)]
#[command(name = "wless")]
struct Cli {
    /// File to view.
    file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let document = Document::open(&cli.file)?;
    let filename = cli
        .file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| cli.file.to_string_lossy().to_string());

    let mut guard = TerminalGuard::new()?;
    let size = guard.terminal.size()?;
    let mut app = AppState::new(document, filename, size.width, size.height);

    while !app.should_quit {
        if app.dirty {
            guard.terminal.draw(|f| render::draw(f, &app))?;
            app.dirty = false;
        }
        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) => {
                    if let Some(action) = input::map_key(key) {
                        app.handle_action(action);
                    }
                }
                Event::Resize(w, h) => app.handle_resize(w, h),
                _ => {}
            }
        }
    }

    Ok(())
}
