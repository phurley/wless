use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// Search history entries kept in memory and persisted; old entries are
/// dropped from the front once this many accumulate, so the config file
/// and the in-session history can't grow without bound over a long-lived
/// habit of searching.
pub const MAX_SEARCH_HISTORY: usize = 200;

/// How many files' last-viewed positions to remember; the least recently
/// exited file is evicted first once this many accumulate.
pub const MAX_RECENT_FILES: usize = 50;

fn default_auto_scroll_interval_ms() -> u64 {
    crate::app::AUTO_SCROLL_DEFAULT_INTERVAL.as_millis() as u64
}

/// The last-viewed line in one file, keyed by its path exactly as it was
/// given on the command line -- no canonicalization or symlink resolution,
/// so a different-but-equivalent path to the same file is treated as a
/// different entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentFile {
    pub path: PathBuf,
    pub line: usize,
}

/// Settings persisted across runs in `~/.config/wless/config.toml`:
/// search history, the last-used auto-scroll speed, and the last-viewed
/// line of recently-opened files. Anything else session-specific (which
/// file is *currently* open, whether auto-scroll/follow happen to be on
/// right now) is deliberately not part of this -- a fresh run should
/// always start in plain view mode.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub search_history: Vec<String>,
    #[serde(default = "default_auto_scroll_interval_ms")]
    pub auto_scroll_interval_ms: u64,
    #[serde(default)]
    pub recent_files: Vec<RecentFile>,
}

impl Default for Config {
    // A hand-derived Default would set auto_scroll_interval_ms to 0 (u64's
    // default), not the intended speed -- it doesn't know about the serde
    // default function above, which only fires for a *missing key* in an
    // existing file, not for "no file at all".
    fn default() -> Self {
        Config {
            search_history: Vec::new(),
            auto_scroll_interval_ms: default_auto_scroll_interval_ms(),
            recent_files: Vec::new(),
        }
    }
}

impl Config {
    fn path() -> Option<PathBuf> {
        Some(
            dirs::home_dir()?
                .join(".config")
                .join("wless")
                .join("config.toml"),
        )
    }

    /// Load the config file, falling back to defaults if it's missing,
    /// unreadable, or fails to parse -- a corrupt config should never
    /// prevent the pager from starting.
    pub fn load() -> Config {
        Self::path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn auto_scroll_interval(&self) -> Duration {
        Duration::from_millis(self.auto_scroll_interval_ms)
    }

    /// The last-viewed line for `path`, if we have one on record for that
    /// exact path string.
    pub fn line_for(&self, path: &std::path::Path) -> Option<usize> {
        self.recent_files
            .iter()
            .find(|rf| rf.path == path)
            .map(|rf| rf.line)
    }

    /// Best-effort save: a read-only home directory or similar shouldn't
    /// be treated as a hard error on exit, so failures are swallowed by
    /// the caller rather than propagated as a user-facing error.
    pub fn save(&self) -> anyhow::Result<()> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }
}
