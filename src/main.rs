mod app;
mod document;
mod follow;
mod input;
mod render;
mod search;
mod terminal;
mod view;
mod wrap;

use app::{AppState, InputMode};
use clap::{ArgAction, Parser};
use crossterm::event::{self, Event};
use document::Document;
use std::path::PathBuf;
use std::time::Duration;
use terminal::TerminalGuard;

/// wless -- a word-wrapping, auto-following pager.
#[derive(Parser)]
#[command(name = "wless", version, disable_version_flag = true)]
struct Cli {
    /// File to view.
    file: PathBuf,

    /// Print version information.
    #[arg(short = 'v', long = "version", action = ArgAction::Version)]
    version: (),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let document = Document::open(&cli.file)?;
    let filename = cli
        .file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| cli.file.to_string_lossy().to_string());
    let watcher = follow::watch(&cli.file)?;

    let mut guard = TerminalGuard::new()?;
    let size = guard.terminal.size()?;
    let mut app = AppState::new(document, cli.file, filename, size.width, size.height);

    while !app.should_quit {
        if app.dirty {
            guard.terminal.draw(|f| render::draw(f, &app))?;
            app.dirty = false;
        }
        if event::poll(Duration::from_millis(150))? {
            match event::read()? {
                Event::Key(key) => match app.input_mode {
                    InputMode::Help => app.close_help(),
                    InputMode::Search(_) => app.handle_search_key(key),
                    InputMode::Normal => {
                        if let Some(action) = input::map_key(key) {
                            app.handle_action(action);
                        }
                    }
                },
                Event::Resize(w, h) => app.handle_resize(w, h),
                _ => {}
            }
        }
        while watcher.rx.try_recv().is_ok() {
            app.handle_file_changed();
        }
    }

    Ok(())
}
