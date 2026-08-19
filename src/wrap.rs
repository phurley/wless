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
/// width (in columns). Wraps at grapheme-cluster boundaries using display
/// width, so wide CJK glyphs and combining marks are handled correctly. An
/// empty line still produces one empty row, matching how a blank line
/// should occupy one screen row.
pub fn wrap_line(text: &str, width: u16) -> Vec<VisualRow> {
    let width = width.max(1) as usize;
    if text.is_empty() {
        return vec![VisualRow { start: 0, end: 0 }];
    }

    let mut rows = Vec::new();
    let mut row_start = 0usize;
    let mut col = 0usize;

    for g in text.grapheme_indices(true) {
        let (byte_idx, grapheme) = g;
        let gw = grapheme.width();
        if col + gw > width && byte_idx > row_start {
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
        col += gw;
    }
    rows.push(VisualRow {
        start: row_start,
        end: text.len(),
    });
    rows
}
