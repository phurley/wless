# wless

A `less`-like pager for the terminal, written in Rust, with three differences
from `less`:

- **Always word-wraps** to the current terminal width -- no horizontal
  scrolling, no chopped lines.
- **Reflows on resize** without losing your place: scroll position is
  tracked as a location in the file, not a screen row, so resizing the
  terminal keeps the same line pinned at the top.
- **Follows file changes** (`F`), like `tail -f` / `less +F`, watching the
  file on disk via [`notify`](https://docs.rs/notify) rather than polling.

It only views one file at a time, given as a path -- no stdin support.

## Install

Download a prebuilt binary from the
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
| `Esc`                   | cancel search / clear highlight     |
| `F`                     | jump to end and follow file changes |
| `Ctrl-L` / `r`          | force redraw                        |
| `h` / `H`               | toggle help overlay                 |
| `q`                     | quit                                |

Pressing any movement key that scrolls away from the bottom (up, page up,
half page up, go to top, or starting a new search) cancels follow mode,
matching `less +F` -- press `F` again to resume.

## Design notes

- Files are assumed to always fit comfortably in memory (even a
  continuously-growing log file), so `wless` reads the whole file up front
  and builds a complete line index in one pass -- no lazy/partial indexing.
  Jumping to an arbitrary line number is intentionally not supported.
- Follow mode re-reads only the appended tail bytes on each file-change
  notification (not the whole file), extending the line index incrementally.
  A shrunk or rotated file (different inode/file id, or a smaller size) is
  detected and triggers a full reload instead.
- Search uses `regex::bytes::Regex` so it works correctly on non-UTF-8
  content without a decode step; displayed text is a lossy UTF-8 decode.

## Building and testing locally

```sh
cargo build
cargo test
cargo clippy --all-targets
cargo fmt
```

## License

MIT -- see [LICENSE](LICENSE).
