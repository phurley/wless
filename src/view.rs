use crate::document::Document;
use crate::wrap::{VisualRow, wrap_line};

/// Where the top of the screen currently sits, expressed as a position in
/// the logical file rather than a visual row count -- this is what lets a
/// resize preserve the user's place even though the number of visual rows
/// a line occupies changes with terminal width.
#[derive(Debug, Clone, Copy)]
pub struct ScrollAnchor {
    pub line_idx: usize,
    pub sub_row: usize,
}

impl ScrollAnchor {
    pub fn top() -> Self {
        ScrollAnchor {
            line_idx: 0,
            sub_row: 0,
        }
    }
}

#[allow(dead_code)] // wired up in the follow-mode milestone
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollMode {
    Fixed,
    BottomFollow,
}

/// One row worth of rendered content: which logical line it came from, and
/// the byte range within that line's text.
pub struct RenderedRow {
    pub line_idx: usize,
    pub row: VisualRow,
}

/// Resolve an anchor into the rows that should be displayed for a screen
/// `height` rows tall and `width` columns wide.
pub fn visible_rows(
    doc: &Document,
    anchor: ScrollAnchor,
    width: u16,
    height: u16,
) -> Vec<RenderedRow> {
    let mut out = Vec::with_capacity(height as usize);
    let mut line_idx = anchor.line_idx.min(doc.last_line_index());
    let mut skip = anchor.sub_row;

    while out.len() < height as usize && line_idx < doc.line_count() {
        let text = doc.line_text(line_idx);
        let rows = wrap_line(&text, width);
        for row in rows.into_iter().skip(skip) {
            out.push(RenderedRow { line_idx, row });
            if out.len() >= height as usize {
                break;
            }
        }
        skip = 0;
        line_idx += 1;
    }
    out
}

/// Total visual rows a logical line occupies at the given width.
fn row_count(doc: &Document, line_idx: usize, width: u16) -> usize {
    wrap_line(&doc.line_text(line_idx), width).len().max(1)
}

pub fn scroll_down_lines(
    doc: &Document,
    anchor: ScrollAnchor,
    width: u16,
    n: usize,
) -> ScrollAnchor {
    let mut line_idx = anchor.line_idx;
    let mut sub_row = anchor.sub_row;
    let last = doc.last_line_index();

    for _ in 0..n {
        let rows_here = row_count(doc, line_idx, width);
        if sub_row + 1 < rows_here {
            sub_row += 1;
        } else if line_idx < last {
            line_idx += 1;
            sub_row = 0;
        } else {
            break;
        }
    }
    ScrollAnchor { line_idx, sub_row }
}

pub fn scroll_up_lines(doc: &Document, anchor: ScrollAnchor, width: u16, n: usize) -> ScrollAnchor {
    let mut line_idx = anchor.line_idx;
    let mut sub_row = anchor.sub_row;

    for _ in 0..n {
        if sub_row > 0 {
            sub_row -= 1;
        } else if line_idx > 0 {
            line_idx -= 1;
            sub_row = row_count(doc, line_idx, width).saturating_sub(1);
        } else {
            break;
        }
    }
    ScrollAnchor { line_idx, sub_row }
}

pub fn page_down(doc: &Document, anchor: ScrollAnchor, width: u16, height: u16) -> ScrollAnchor {
    let n = (height as usize).saturating_sub(1).max(1);
    scroll_down_lines(doc, anchor, width, n)
}

pub fn page_up(doc: &Document, anchor: ScrollAnchor, width: u16, height: u16) -> ScrollAnchor {
    let n = (height as usize).saturating_sub(1).max(1);
    scroll_up_lines(doc, anchor, width, n)
}

pub fn goto_top() -> ScrollAnchor {
    ScrollAnchor::top()
}

pub fn goto_bottom(doc: &Document, width: u16, height: u16) -> ScrollAnchor {
    // Walk backward from EOF accumulating visual rows until we've filled
    // one screen, so the last line of the file lands on the last row.
    let mut line_idx = doc.last_line_index();
    let mut rows_needed = height as usize;
    let first_line;
    let first_line_rows;

    loop {
        let rows_here = row_count(doc, line_idx, width);
        if rows_here >= rows_needed {
            first_line = line_idx;
            first_line_rows = rows_here;
            break;
        }
        rows_needed -= rows_here;
        if line_idx == 0 {
            first_line = 0;
            first_line_rows = row_count(doc, 0, width);
            break;
        }
        line_idx -= 1;
    }

    let sub_row = first_line_rows.saturating_sub(rows_needed);
    ScrollAnchor {
        line_idx: first_line,
        sub_row,
    }
}

/// Byte offset of the anchor's logical line, for status-bar percentage math.
pub fn anchor_offset(doc: &Document, anchor: ScrollAnchor) -> u64 {
    doc.line_offset(anchor.line_idx.min(doc.last_line_index()))
}

/// Whether the current screen already shows the end of the file, i.e.
/// scrolling down further would not reveal anything new. Used to decide
/// when the status bar should show "END" instead of a percentage.
pub fn is_at_bottom(doc: &Document, anchor: ScrollAnchor, width: u16, height: u16) -> bool {
    match visible_rows(doc, anchor, width, height).last() {
        None => true,
        Some(last) => {
            last.line_idx == doc.last_line_index()
                && last.row.end == doc.line_text(last.line_idx).len()
        }
    }
}

/// Recompute an anchor after a resize so it keeps pointing at the same
/// logical content, clamping sub_row to the new wrap's row count.
pub fn reflow_anchor(doc: &Document, anchor: ScrollAnchor, new_width: u16) -> ScrollAnchor {
    let line_idx = anchor.line_idx.min(doc.last_line_index());
    let rows = row_count(doc, line_idx, new_width);
    ScrollAnchor {
        line_idx,
        sub_row: anchor.sub_row.min(rows.saturating_sub(1)),
    }
}
