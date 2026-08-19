# wless

A `less`-like pager for the terminal, written in Rust, with three differences
from `less`:

- **Always word-wraps** to the current terminal width -- no horizontal
  scrolling, no chopped lines.
- **Reflows on resize** without losing your place: scroll position is
  tracked as a location in the file, not a screen row, so resizing the
  terminal keeps the same line pinned at the top.
- **Follows file changes** (`F`, or start with `-f`), like `tail -f` /
  `less +F`, by polling the file's size roughly every 150ms --
  deliberately not OS-level file change notifications, which proved
  unreliable in practice on at least one real setup.

It only views one file at a time, given as a path -- no stdin support.

## Install

macOS or Linux, via [Homebrew](https://brew.sh):

```sh
brew install phurley/wless/wless
```

Or download a prebuilt binary from the
[releases page](https://github.com/phurley/wless/releases), or build from
source with Cargo:

```sh
cargo install --git https://github.com/phurley/wless
```

or, from a local checkout:

```sh
git clone https://github.com/phurley/wless
cd wless
cargo install --path .
```

## Usage

```sh
wless <file>
wless -a <file>   # start with auto-scroll already running
wless -f <file>   # start already following, like tail -f
```

## Keybindings

| Key                    | Action                              |
| ----------------------- | ------------------------------------ |
| `Up` / `k`              | scroll up one line                  |
| `Down` / `j` / `Enter`  | scroll down one line                |
| `Space` / `f` / `PgDn`  | page down                           |
| `b` / `PgUp`            | page up                             |
| `Ctrl-D` / `Ctrl-U`     | half page down / up                 |
| `g` / `Home`            | go to top                           |
| `G` / `End`             | go to bottom                        |
| `/`                     | search forward (regex)              |
| `?`                     | search backward (regex)             |
| Enter on empty prompt   | repeat the last search pattern      |
| `Up` / `Down` in prompt | browse search history               |
| `n` / `N`               | repeat search (same / opposite dir) |
| `i`                     | toggle case-sensitive search        |
| `Esc`                   | cancel search / clear highlight     |
| `F`                     | jump to end and follow file changes |
| `a`                     | toggle auto-scroll (teleprompter)   |
| `+` / `-`               | auto-scroll speed up / down         |
| `m`                     | toggle markdown styling             |
| `Ctrl-L` / `r`          | force redraw                        |
| `h` / `H`               | toggle help overlay                 |
| `q`                     | quit                                |

Pressing any movement key that scrolls away from the bottom (up, page up,
half page up, go to top, or starting a new search) cancels follow mode,
matching `less +F` -- press `F` again to resume.

While auto-scrolling, `Up`/`Down` do double duty: they still move a line
as usual, but also nudge the speed down/up, so a manual nudge is coupled
to the running pace instead of feeling disconnected from it. Reaching the
current end of file while auto-scrolling hands off to follow mode, so it
keeps riding newly appended content.

Auto-scroll pauses (without turning off) while the search prompt or the
help overlay is open, so it can't scroll the view out from under you
while you type -- searching with `/`/`?`/`n`/`N` while auto-scrolling
just jumps to the match and picks the pace back up from there.

Search is case-insensitive by default; `i` toggles it (the search prompt
shows a `(CS)` marker only while case-sensitive is active, to keep the
common case uncluttered). The choice is remembered per file, the same way
last-viewed position is (see Persisted settings below) -- toggling it for
one file doesn't affect any other file's default.

## Markdown styling

`wless` can apply very limited terminal styling on top of markdown text --
purely visual, layered on the literal, unmodified line text: no punctuation
(`**`, `#`, `-`, etc.) is ever stripped, and line length / word-wrapping is
completely unaffected. In scope: `# Header` lines rendered bold,
`**bold**`/`__bold__` and `*italic*`/`_italic_` spans (bold takes priority
over italic), and the leading marker of bullet (`-`/`*`/`+`) and numbered
(`1.`) list items rendered bold. Nothing else -- no code spans, blockquotes,
links, or nested emphasis.

Styling is auto-enabled for files with a `.md` or `.markdown` extension
(case-insensitive) and off otherwise. Press `m` to toggle it for the
currently-open file; your choice is remembered per file (the same exact
path-string matching used for the last-viewed-line memory above), so a
`.md` file you've never explicitly toggled keeps following extension
auto-detection forever, while one you have toggled stays at your choice.

## Design notes

- Files are assumed to always fit comfortably in memory (even a
  continuously-growing log file), so `wless` reads the whole file up front
  and builds a complete line index in one pass -- no lazy/partial indexing.
  Jumping to an arbitrary line number is intentionally not supported.
- Follow mode re-reads only the appended tail bytes on each detected
  change (not the whole file), extending the line index incrementally.
  A shrunk or rotated file (different inode/file id, or a smaller size) is
  detected and triggers a full reload instead. Detection is a cheap
  `stat()`-based size check every ~150ms, not OS file-change
  notifications -- simpler, and can't silently fail to fire the way those
  did on at least one real machine.
- Search uses `regex::bytes::Regex` so it works correctly on non-UTF-8
  content without a decode step; displayed text is a lossy UTF-8 decode.

## Persisted settings

Search history, the last-used auto-scroll speed, and (per file) the
last-viewed line, case-sensitivity choice, and markdown-styling choice
(if you've ever toggled it with `m`) of up to 50 recently-opened files are
saved to `~/.config/wless/config.toml` on exit and reloaded on the next
run. Reopening a file restores its remembered state if -- and only if --
it's given by the exact same path string as before (no canonicalization
or symlink resolution). Whether follow/auto-scroll happen to be on is not
persisted; every run starts in plain view mode.

### Theme

Colors are configurable via a `[theme]` section in `config.toml` --
add one by hand (defaults shown; any subset of keys can be overridden,
using [ratatui's color
names](https://docs.rs/ratatui/latest/ratatui/style/enum.Color.html) like
`"Yellow"`, `"Cyan"`, `"LightBlue"`, or `{ Rgb = [255, 0, 0] }`):

```toml
[theme]
search_match_bg = "Yellow"
search_match_fg = "Black"
status_bg = "DarkGray"
status_fg = "White"
```

## Building and testing locally

```sh
cargo build
cargo test
cargo clippy --all-targets
cargo fmt
```

## License

MIT -- see [LICENSE](LICENSE).
