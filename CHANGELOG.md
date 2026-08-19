# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.1] - 2026-08-19

### Fixed

- Follow mode (`F`) could silently never pick up new content: it relied
  on OS-level file-change notifications (`notify`), which turned out to
  be unreliable in practice on at least one real setup -- a concurrent
  `tail -f` on the same file saw appends immediately while wless saw
  nothing. Replaced with the same strategy `tail -f` itself falls back
  to: polling the file's size directly (a single cheap `stat()` call
  when nothing changed) roughly every 150ms. This also removes the
  `notify`/`notify-debouncer-full` dependencies entirely.
- Scrolling down (manually, or via auto-scroll) within one screenful of
  the end of file kept advancing the file's last line up past the bottom
  row, leaving a growing wall of blank lines below it instead of
  stopping like a normal pager -- most visible as auto-scroll appearing
  to "scroll into" empty space at the end of a file rather than stopping
  cleanly at the last line.

## [1.0.0] - 2026-08-19

### Added

- Auto-scroll (teleprompter) mode: `a` toggles it, `+`/`-` adjust speed,
  and reaching the current end of file hands off to follow mode so it
  keeps riding newly appended content. While auto-scrolling, `Up`/`Down`
  both move a line and nudge the speed down/up. A `-a`/`--auto-scroll`
  flag starts a run with it already on.
- Auto-scroll now pauses (without turning off) while the search prompt or
  help overlay is open, instead of silently scrolling the view underneath
  you -- searching while auto-scrolling just jumps to the match and picks
  the pace back up from there.
- Settings are persisted to `~/.config/wless/config.toml` and reloaded on
  the next run: search history, the last-used auto-scroll speed, and the
  last-viewed line of up to 50 recently-opened files (matched by exact
  path string, no canonicalization).

### Fixed

- A crash on submitting an empty search query, caused by `regex::bytes`
  zero-width matches landing mid-character; an empty query now repeats
  the last pattern instead of ever compiling `""`.
- `refresh_append` (follow mode) wasn't recording a line-start for the
  first new line after an append when the file previously ended cleanly
  with `\n`, so appended lines rendered glued onto the previous line.
- Windows build was broken (an unstable std API, then a Unix-only one);
  fixed by switching file-identity comparison to the `same-file` crate.

## [0.3.0] - 2026-08-19

Initial release.

### Added

- Word-wrapping text viewer with a scroll position tracked by file
  location (not screen row), so resizing the terminal reflows the text
  without losing your place.
- Standard navigation: arrow keys / `j`/`k`, `Space`/`f`/`b`/page up/down,
  `Ctrl-D`/`Ctrl-U` half-page, `g`/`G` to jump to top/bottom.
- Regex search (`/` forward, `?` backward, `n`/`N` to repeat), with all
  on-screen matches highlighted, a search history browsable with
  `Up`/`Down`, and an empty query repeating the last pattern.
- Follow mode (`F`), watching the file via `notify` and auto-scrolling as
  it grows; any manual scroll away from the bottom cancels it.
- Help overlay (`h`/`H`).
- `-v`/`--version` flag.
