use crate::app::{AppState, InputMode};
use crate::markdown::{self, MdKind};
use crate::search;
use crate::view::visible_rows;
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use std::ops::Range;

/// Per-logical-line memo of computed markdown spans: which line they were
/// computed for, and the spans themselves.
type MdMemo = Option<(usize, Vec<(Range<usize>, MdKind)>)>;

const HELP_TEXT: &str = "\
wless -- word-wrapping, auto-following pager

Navigation
  Up / k              scroll up one line
  Down / j / Enter     scroll down one line
  Space / f / PgDn    page down
  b / PgUp            page up
  Ctrl-D / Ctrl-U     half page down / up
  g / Home            go to top
  G / End             go to bottom

Search
  /                   search forward (regex)
  ?                   search backward (regex)
  Enter (empty)       repeat last pattern
  Up / Down           browse search history
  n / N               repeat search (same / opposite direction)
  i                   toggle case-sensitive search (default: insensitive)
  Esc                 cancel search / clear highlight

Follow
  F                   jump to end and follow appended content

Auto-scroll (teleprompter)
  a                   toggle auto-scroll
  + / -               speed up / slow down
  Up / Down           while auto-scrolling: also nudges speed
                      down / up (in addition to moving a line)
                      searching jumps to the match, then resumes

Markdown
  m                   toggle markdown styling (bold/italic only --
                      punctuation stays on screen, nothing is
                      stripped or rewrapped)
                      auto-enabled for .md/.markdown files; your
                      choice is remembered per file

Other
  Ctrl-L / r          force redraw
  h / H               toggle this help
  q                   quit

Press any key to close";

pub fn draw(frame: &mut Frame, app: &AppState) {
    let area = frame.area();
    let [text_area, status_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    draw_text(frame, app, text_area);
    draw_status(frame, app, status_area, text_area);

    if matches!(app.input_mode, InputMode::Help) {
        draw_help(frame, area);
    }
}

fn draw_text(frame: &mut Frame, app: &AppState, area: Rect) {
    let rows = visible_rows(&app.document, app.anchor, area.width, area.height);
    let match_style = Style::default()
        .bg(app.theme.search_match_bg)
        .fg(app.theme.search_match_fg);

    // A wrapped line's markdown spans and search matches are identical
    // across all of its visual rows, so each is only recomputed when the
    // logical line actually changes -- `visible_rows` always emits one
    // line's rows contiguously, so a single "last computed for" memo is
    // enough.
    let mut md_memo: MdMemo = None;
    let mut search_memo: Option<(usize, Vec<Range<usize>>)> = None;

    let lines: Vec<Line> = rows
        .iter()
        .map(|r| {
            let text = app.document.line_text(r.line_idx);

            let md_spans: &[(Range<usize>, MdKind)] = if app.markdown_enabled {
                if md_memo.as_ref().map(|(idx, _)| *idx) != Some(r.line_idx) {
                    md_memo = Some((r.line_idx, markdown::markdown_spans(&text)));
                }
                &md_memo.as_ref().unwrap().1
            } else {
                &[]
            };

            let search_matches: &[Range<usize>] = match &app.last_pattern {
                Some(re) => {
                    if search_memo.as_ref().map(|(idx, _)| *idx) != Some(r.line_idx) {
                        search_memo = Some((
                            r.line_idx,
                            search::find_all_in_line(re, app.document.line_bytes(r.line_idx)),
                        ));
                    }
                    &search_memo.as_ref().unwrap().1
                }
                None => &[],
            };

            styled_line(
                &text,
                r.row.start,
                r.row.end,
                md_spans,
                search_matches,
                match_style,
            )
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).block(Block::default()), area);
}

/// Round `idx` outward to the nearest valid char boundary of `text`, never
/// past `[lo, hi]`. Regex matches on raw bytes aren't guaranteed to land on
/// char boundaries (e.g. an empty pattern's zero-width matches occur at
/// every byte offset, including mid-character), so match ranges must be
/// snapped before they're used to slice the `&str` used for display.
fn floor_boundary(text: &str, idx: usize) -> usize {
    let mut idx = idx.min(text.len());
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn ceil_boundary(text: &str, idx: usize) -> usize {
    let mut idx = idx.min(text.len());
    while idx < text.len() && !text.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

/// The terminal styling for one kind of "very limited" markdown span. Only
/// sets the BOLD/ITALIC modifier -- disjoint from the search layer's bg/fg,
/// so the two combine cleanly via `Style::patch` regardless of merge order.
fn markdown_style(kind: MdKind) -> Style {
    match kind {
        MdKind::Header | MdKind::Bold | MdKind::Marker => {
            Style::default().add_modifier(Modifier::BOLD)
        }
        MdKind::Italic => Style::default().add_modifier(Modifier::ITALIC),
    }
}

/// Build a `Line` for one visual row (`text[row_start..row_end]`), merging
/// two style layers -- markdown spans and search matches, both given as
/// byte ranges relative to the full logical line -- into one set of
/// non-overlapping spans. Overlapping regions from both layers combine
/// (e.g. a bold word that's also a search hit shows bold text on a
/// highlighted background) via a boundary-point sweep: every layer range's
/// start/end becomes a cut point, and each resulting sub-interval folds
/// together (`Style::patch`) every layer range that fully covers it.
///
/// Markdown ranges come from a str-based regex on the already-lossily-
/// decoded line text, so they're guaranteed to land on char boundaries and
/// only need row-clipping. Search ranges come from `regex::bytes::Regex`
/// matches, which are not guaranteed char-boundary-aligned, so they still
/// need the `floor_boundary`/`ceil_boundary` snapping below.
fn styled_line(
    text: &str,
    row_start: usize,
    row_end: usize,
    md_spans: &[(std::ops::Range<usize>, MdKind)],
    search_matches: &[std::ops::Range<usize>],
    search_style: Style,
) -> Line<'static> {
    let mut layers: Vec<(std::ops::Range<usize>, Style)> = Vec::new();
    for (r, kind) in md_spans {
        let start = r.start.max(row_start);
        let end = r.end.min(row_end);
        if start < end {
            layers.push((start..end, markdown_style(*kind)));
        }
    }
    for m in search_matches {
        let start = ceil_boundary(text, m.start.max(row_start));
        let end = floor_boundary(text, m.end.min(row_end));
        if start < end {
            layers.push((start..end, search_style));
        }
    }

    if layers.is_empty() {
        return Line::from(text[row_start..row_end].to_string());
    }

    let mut boundaries: Vec<usize> = vec![row_start, row_end];
    for (r, _) in &layers {
        boundaries.push(r.start);
        boundaries.push(r.end);
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut spans = Vec::new();
    for w in boundaries.windows(2) {
        let (seg_start, seg_end) = (w[0], w[1]);
        if seg_start >= seg_end {
            continue;
        }
        let mut style = Style::default();
        let mut any = false;
        for (r, s) in &layers {
            if r.start <= seg_start && seg_end <= r.end {
                style = style.patch(*s);
                any = true;
            }
        }
        let segment = text[seg_start..seg_end].to_string();
        spans.push(if any {
            Span::styled(segment, style)
        } else {
            Span::raw(segment)
        });
    }
    Line::from(spans)
}

fn draw_status(frame: &mut Frame, app: &AppState, area: Rect, text_area: Rect) {
    if let InputMode::Search(state) = &app.input_mode {
        // Only show a case-sensitivity marker in the non-default state, so
        // the common case (case-insensitive) stays visually uncluttered.
        let case_marker = if app.ignore_case { "" } else { "(CS)" };
        let text = format!(
            "{}{}{}",
            state.direction.prompt_char(),
            case_marker,
            state.query
        );
        frame.render_widget(Paragraph::new(text), area);
        return;
    }

    let text = if let Some(msg) = &app.status_message {
        format!(" {}", msg)
    } else {
        let pct = if app.document.len() == 0 {
            100
        } else {
            let offset = crate::view::anchor_offset(&app.document, app.anchor);
            ((offset as f64 / app.document.len() as f64) * 100.0) as u64
        };
        let at_end =
            crate::view::is_at_bottom(&app.document, app.anchor, text_area.width, text_area.height);
        let pos = if at_end {
            "END".to_string()
        } else {
            format!("{pct}%")
        };
        let auto = if app.auto_scroll {
            let lines_per_sec = 1000.0 / app.auto_scroll_interval.as_millis().max(1) as f64;
            format!("  [AUTO {lines_per_sec:.1}/s]")
        } else {
            String::new()
        };
        format!(" {}  ({}){}", app.filename, pos, auto)
    };
    let style = Style::default()
        .bg(app.theme.status_bg)
        .fg(app.theme.status_fg);
    let line = Line::from(Span::styled(text, style));
    frame.render_widget(Paragraph::new(line).style(style), area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let lines = HELP_TEXT.lines().count() as u16 + 2;
    let width = HELP_TEXT.lines().map(|l| l.len()).max().unwrap_or(0) as u16 + 4;
    let [popup] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [popup] = Layout::vertical([Constraint::Length(lines.min(area.height))])
        .flex(Flex::Center)
        .areas(popup);

    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" wless help ")
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let text = Paragraph::new(HELP_TEXT)
        .block(block)
        .style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(text, popup);
}
