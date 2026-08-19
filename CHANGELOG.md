# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
