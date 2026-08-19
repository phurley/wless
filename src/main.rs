mod app;
mod config;
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
use std::time::{Duration, Instant};
use terminal::TerminalGuard;

const BASE_POLL_TIMEOUT: Duration = Duration::from_millis(150);

/// wless -- a word-wrapping, auto-following pager.
#[derive(Parser)]
#[command(name = "wless", version, disable_version_flag = true)]
struct Cli {
    /// File to view.
    file: PathBuf,

    /// Start with auto-scroll (teleprompter mode) already running.
    #[arg(short = 'a', long = "auto-scroll")]
    auto_scroll: bool,

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

    let config = config::Config::load();

    let mut guard = TerminalGuard::new()?;
    let size = guard.terminal.size()?;
    let mut app = AppState::new(document, cli.file, filename, size.width, size.height);
    app.apply_config(&config);
    if cli.auto_scroll {
        app.set_auto_scroll(true);
    }

    while !app.should_quit {
        if app.dirty {
            guard.terminal.draw(|f| render::draw(f, &app))?;
            app.dirty = false;
        }
        let timeout = match app.auto_scroll_wake_deadline() {
            Some(deadline) => deadline
                .saturating_duration_since(Instant::now())
                .min(BASE_POLL_TIMEOUT),
            None => BASE_POLL_TIMEOUT,
        };
        if event::poll(timeout)? {
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
        while app.auto_scroll_due(Instant::now()) {
            app.auto_scroll_tick();
        }
    }

    // Best-effort: a failure to save (e.g. read-only home directory)
    // shouldn't be treated as an error on an otherwise-normal exit.
    let _ = app.to_config().save();

    Ok(())
}
