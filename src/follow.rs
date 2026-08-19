use notify::RecursiveMode;
use notify_debouncer_full::{Debouncer, RecommendedCache, new_debouncer};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

/// Watches one file on disk and signals via a channel whenever it changes.
/// Uses a debounced watcher so a burst of rapid appends (e.g. a fast log
/// writer) collapses into a single notification instead of flooding the
/// event loop.
pub struct FileWatcher {
    // Held only to keep the background watcher thread alive for the
    // lifetime of the app; never read directly.
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
    pub rx: mpsc::Receiver<()>,
}

pub fn watch(path: &Path) -> anyhow::Result<FileWatcher> {
    let (tx, rx) = mpsc::channel();
    let mut debouncer = new_debouncer(
        Duration::from_millis(80),
        None,
        move |result: notify_debouncer_full::DebounceEventResult| {
            if result.is_ok() {
                let _ = tx.send(());
            }
        },
    )?;
    // Watching the file path directly (not its parent directory) covers
    // the common case of a process appending to this file in place. It
    // will not notice a rotate-by-rename-and-recreate at the same path on
    // every platform/backend -- refresh_append's identity check still
    // catches that on the next event that *does* arrive, but a rotation
    // with no further writes to the new file could go unnoticed. Watching
    // the parent directory to cover that fully was judged not worth the
    // added complexity for a personal pager.
    debouncer.watch(path, RecursiveMode::NonRecursive)?;
    Ok(FileWatcher {
        _debouncer: debouncer,
        rx,
    })
}
