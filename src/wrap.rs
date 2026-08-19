use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A single visual (post-wrap) row within a logical line, as a byte range
/// relative to the start of that line's text (not the file).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisualRow {
    pub start: usize,
    pub end: usize,
}

/// Safety valve for pathological single-line files (e.g. a multi-MB
/// minified-JSON line): stop wrapping past this many rows so a redraw can't
/// hang, and let the caller show a "truncated" indicator instead.
pub const MAX_ROWS_PER_LINE: usize = 50_000;

/// Wrap one logical line's text into visual rows at the given terminal
/// width (in columns). This is true word-wrap: rows are broken at word
/// boundaries (`unicode-segmentation`'s word-bound tokens, so whitespace
/// runs count as boundaries too) using display width
/// (`unicode-width`), so words are never split mid-token. Only a single
/// word wider than the entire terminal falls back to a hard grapheme-level
/// break, since there is no boundary available to wrap it at. An empty
/// line still produces one empty row, matching how a blank line should
/// occupy one screen row.
pub fn wrap_line(text: &str, width: u16) -> Vec<VisualRow> {
    let width = width.max(1) as usize;
    if text.is_empty() {
        return vec![VisualRow { start: 0, end: 0 }];
    }

    let mut rows = Vec::new();
    let mut row_start = 0usize;
    let mut col = 0usize;

    for (byte_idx, token) in text.split_word_bound_indices() {
        let token_width = token.width();

        if col > 0 && col + token_width > width {
            rows.push(VisualRow {
                start: row_start,
                end: byte_idx,
            });
            row_start = byte_idx;
            col = 0;
            if rows.len() >= MAX_ROWS_PER_LINE {
                return rows;
            }
        }

        if token_width > width {
            // The token itself doesn't fit on a fresh row (e.g. a very long
            // URL or minified identifier) -- there's no word boundary to
            // wrap at, so fall back to a hard break by grapheme cluster.
            let mut sub_col = 0usize;
            let mut sub_start = byte_idx;
            for (gi, g) in token.grapheme_indices(true) {
                let gw = g.width();
                let abs = byte_idx + gi;
                if sub_col + gw > width && abs > sub_start {
                    rows.push(VisualRow {
                        start: sub_start,
                        end: abs,
                    });
                    sub_start = abs;
                    sub_col = 0;
                    if rows.len() >= MAX_ROWS_PER_LINE {
                        return rows;
                    }
                }
                sub_col += gw;
            }
            row_start = sub_start;
            col = sub_col;
            continue;
        }

        col += token_width;
    }
    rows.push(VisualRow {
        start: row_start,
        end: text.len(),
    });
    rows
}
