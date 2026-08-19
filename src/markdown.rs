use regex::Regex;
use std::ops::Range;
use std::path::Path;
use std::sync::LazyLock;

/// The kind of "very limited" markdown styling detected for a span of text.
/// Deliberately minimal: no code spans, blockquotes, links, or nested
/// emphasis beyond what falls out naturally from bold-before-italic
/// priority. See `markdown_spans` for the two-pass algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdKind {
    Header,
    Bold,
    Italic,
    Marker,
}

static HEADER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s").unwrap());
static BULLET_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*([-*+])\s").unwrap());
static NUMBERED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(\d+\.)\s").unwrap());
static BOLD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\*\*(.+?)\*\*|\b__(.+?)__\b").unwrap());
static ITALIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*(.+?)\*|\b_(.+?)_\b").unwrap());

/// Compute the markdown styling spans for one logical line of text, as byte
/// ranges (relative to the start of `text`) paired with the kind of styling
/// to apply. This is a pure, side-effect-free scan over the literal text --
/// it never strips or rewrites any characters, so callers can safely apply
/// the resulting styles on top of the unmodified line while wrapping and
/// byte offsets elsewhere (search, scroll anchors) stay untouched.
///
/// Two-pass by construction so bold always wins over italic: a single
/// alternation regex can't guarantee that, since a naive italic scan
/// starting from an early lone `*` can greedily swallow a later `**` before
/// the engine ever gets a chance to prefer bold at that position.
pub fn markdown_spans(text: &str) -> Vec<(Range<usize>, MdKind)> {
    if HEADER_RE.is_match(text) {
        return vec![(0..text.len(), MdKind::Header)];
    }

    let mut spans = Vec::new();
    let mut scan_start = 0;
    if let Some(caps) = BULLET_RE.captures(text) {
        let m = caps.get(1).unwrap();
        spans.push((m.range(), MdKind::Marker));
        scan_start = m.end();
    } else if let Some(caps) = NUMBERED_RE.captures(text) {
        let m = caps.get(1).unwrap();
        spans.push((m.range(), MdKind::Marker));
        scan_start = m.end();
    }

    let mut bold_ranges: Vec<Range<usize>> = Vec::new();
    for m in BOLD_RE.find_iter(&text[scan_start..]) {
        let r = (scan_start + m.start())..(scan_start + m.end());
        bold_ranges.push(r.clone());
        spans.push((r, MdKind::Bold));
    }

    let mut pos = scan_start;
    for b in &bold_ranges {
        scan_italic_gap(text, pos, b.start, &mut spans);
        pos = b.end;
    }
    scan_italic_gap(text, pos, text.len(), &mut spans);

    spans
}

/// Scan for italic matches strictly within `text[from..to]`, so a match can
/// never straddle into a bold span that was already claimed elsewhere in the
/// line -- this is what makes the bold-then-italic two-pass approach work
/// without any overlap-filtering after the fact.
fn scan_italic_gap(text: &str, from: usize, to: usize, out: &mut Vec<(Range<usize>, MdKind)>) {
    if from >= to {
        return;
    }
    for m in ITALIC_RE.find_iter(&text[from..to]) {
        out.push((from + m.start()..from + m.end(), MdKind::Italic));
    }
}

/// Whether a file's extension suggests markdown content, used to decide the
/// default on/off state for markdown styling when a file is first opened
/// (before any per-file override from the config has been applied).
pub fn detect_by_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_claims_whole_line() {
        let text = "## Section title";
        let spans = markdown_spans(text);
        assert_eq!(spans, vec![(0..text.len(), MdKind::Header)]);
    }

    #[test]
    fn header_requires_whitespace_after_hashes() {
        // `#hashtag`-like text should not be treated as a header.
        let spans = markdown_spans("#nothash rest of line");
        assert!(!spans.iter().any(|(_, k)| *k == MdKind::Header));
    }

    #[test]
    fn bold_wins_over_overlapping_italic() {
        let text = "*a **b** c*";
        let spans = markdown_spans(text);
        // The real bold span is "**b**" starting at byte 3.
        let bold_start = text.find("**b**").unwrap();
        assert!(
            spans
                .iter()
                .any(|(r, k)| *k == MdKind::Bold && r.start == bold_start)
        );
        // No italic span should overlap the bold span.
        let bold_range = bold_start..(bold_start + "**b**".len());
        for (r, k) in &spans {
            if *k == MdKind::Italic {
                assert!(r.end <= bold_range.start || r.start >= bold_range.end);
            }
        }
    }

    #[test]
    fn bullet_marker_then_inline_italic() {
        let text = "* just italic *word* here";
        let spans = markdown_spans(text);
        // Marker should be just the leading "*".
        assert!(
            spans
                .iter()
                .any(|(r, k)| *k == MdKind::Marker && *r == (0..1))
        );
        // The italic span should be "*word*", found after the marker, not
        // starting at byte 0 (which would incorrectly pair the marker's `*`
        // with the first `*` of "*word*").
        let italic_start = text.find("*word*").unwrap();
        assert!(
            spans
                .iter()
                .any(|(r, k)| *k == MdKind::Italic && r.start == italic_start)
        );
    }

    #[test]
    fn numbered_marker() {
        let text = "42. an item";
        let spans = markdown_spans(text);
        assert!(
            spans
                .iter()
                .any(|(r, k)| *k == MdKind::Marker && *r == (0..3))
        );
    }

    #[test]
    fn underscore_word_boundary_avoids_snake_case_false_positive() {
        let text = "use my_variable_name here";
        let spans = markdown_spans(text);
        assert!(!spans.iter().any(|(_, k)| *k == MdKind::Italic));
    }

    #[test]
    fn dunder_bold_still_detected() {
        let text = "a __bold__ word";
        let spans = markdown_spans(text);
        assert!(spans.iter().any(|(_, k)| *k == MdKind::Bold));
    }

    #[test]
    fn detect_by_extension_matches_md_and_markdown_case_insensitive() {
        assert!(detect_by_extension(Path::new("notes.md")));
        assert!(detect_by_extension(Path::new("notes.MARKDOWN")));
        assert!(!detect_by_extension(Path::new("notes.txt")));
        assert!(!detect_by_extension(Path::new("notes")));
    }
}
