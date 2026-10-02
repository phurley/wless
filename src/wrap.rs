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

    let units = wrap_units(text);

    let mut rows = Vec::new();
    let mut row_start = 0usize;
    let mut col = 0usize;

    for (byte_idx, unit_end) in units {
        let token = &text[byte_idx..unit_end];
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

/// A closing punctuation mark that should stay attached to the word it
/// follows rather than being pushed to the start of the next visual row.
fn is_closing_punct(c: char) -> bool {
    matches!(
        c,
        '.' | ','
            | ';'
            | ':'
            | '!'
            | '?'
            | '%'
            | ')'
            | ']'
            | '}'
            | '"'
            | '\''
            | '\u{2019}'
            | '\u{201D}'
            | '\u{00BB}'
    )
}

/// Split a line into wrap units: word-boundary tokens with any immediately
/// following closing punctuation folded into the preceding unit. The result
/// is a list of `(start, end)` byte ranges covering `text` in order.
fn wrap_units(text: &str) -> Vec<(usize, usize)> {
    let mut units: Vec<(usize, usize)> = Vec::new();
    // Start-of-line counts as whitespace so a leading quote is not treated
    // as closing punctuation.
    let mut prev_was_space = true;

    for (byte_idx, token) in text.split_word_bound_indices() {
        let is_space = token.chars().all(char::is_whitespace);
        let attach = !prev_was_space
            && !is_space
            && token.chars().all(is_closing_punct)
            && units.last().is_some_and(|&(_, end)| end == byte_idx);

        if attach {
            if let Some(last) = units.last_mut() {
                last.1 = byte_idx + token.len();
            }
        } else {
            units.push((byte_idx, byte_idx + token.len()));
        }
        prev_was_space = is_space;
    }
    units
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(text: &str, width: u16) -> Vec<&str> {
        wrap_line(text, width)
            .into_iter()
            .map(|r| &text[r.start..r.end])
            .collect()
    }

    #[test]
    fn period_stays_with_preceding_word() {
        // "abcd def" exactly fills the row, so the trailing '.' must move
        // down with "def" instead of starting the next row on its own.
        assert_eq!(rows("abcd def.", 8), vec!["abcd ", "def."]);
    }

    #[test]
    fn comma_stays_with_preceding_word() {
        assert_eq!(rows("abcd def,", 8), vec!["abcd ", "def,"]);
    }

    #[test]
    fn closing_quote_stays_with_preceding_word() {
        assert_eq!(rows("abcd def.\"", 8), vec!["abcd ", "def.\""]);
    }

    #[test]
    fn rows_never_start_with_closing_punctuation() {
        // Widths below the longest word+punctuation fall back to a hard
        // grapheme break, which is allowed to split punctuation off.
        for width in 6..=12 {
            for text in [
                "Hello, world.",
                "one two three, four.",
                "say \"go.",
                "a (b) c,",
            ] {
                for row in rows(text, width) {
                    let first = row.chars().next();
                    assert!(
                        !first.is_some_and(is_closing_punct),
                        "row {row:?} starts with punctuation (text {text:?}, width {width})"
                    );
                }
            }
        }
    }
}
